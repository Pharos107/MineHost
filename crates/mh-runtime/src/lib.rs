//! Chạy server dưới dạng process con (spec P1, mục 5.5). Crate này không biết gì về provider:
//! nó chỉ nhận chương trình, tham số và thư mục làm việc.

mod complete;
mod launch;
mod os;
mod spawner;

use std::path::PathBuf;
use std::time::Duration;

use mh_core::{ConsoleStream, ErrorCode};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::runtime::Handle;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

pub use complete::Completion;
pub use launch::{ENCODING_FLAGS, JavaLaunch};

/// Sau khi process thoát, chờ tối đa chừng này để đọc nốt log còn trong pipe. Process cháu
/// có thể giữ pipe mở mãi, nên không chờ vô hạn.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

/// Dòng dài hơn chừng này bị cắt. Mod in cả registry trên một dòng, hay dữ liệu nhị phân không có
/// xuống dòng, sẽ làm bộ đệm đọc và bộ đệm console lớn vô hạn.
pub const MAX_LINE_BYTES: usize = 16 * 1024;
/// Số dòng chờ người đọc xử lý. Đầy thì bỏ bớt và báo số dòng đã bỏ, nhưng không bao giờ ngừng đọc
/// pipe: ngừng đọc thì server bị treo ngay khi ghi log.
pub const OUTPUT_QUEUE: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub work_dir: PathBuf,
    /// Giới hạn OS áp cho process và mọi process con của nó (spec P3, mục 3.8).
    pub limits: Option<ResourceLimits>,
}

/// Windows áp bằng Job Object riêng của process; Linux bằng cgroup v2 khi được giao quyền
/// (`memory.max`; `pids.max` đếm cả luồng nên dùng mức riêng, không dùng `max_processes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    pub memory_bytes: u64,
    pub max_processes: u32,
}

impl ResourceLimits {
    /// Java thường chỉ có một process; chừa chỗ cho plugin gọi chương trình ngoài.
    pub const MAX_PROCESSES: u32 = 16;

    pub const fn for_heap_mb(max_heap_mb: u32) -> Self {
        Self {
            memory_bytes: mh_core::memory_limit_mb(max_heap_mb) * 1024 * 1024,
            max_processes: Self::MAX_PROCESSES,
        }
    }
}

/// Một dòng stdout/stderr, đã giải mã UTF-8 kiểu lossy và bỏ ký tự xuống dòng.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputLine {
    pub stream: ConsoleStream,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exit {
    /// `None` khi process bị kill bằng tín hiệu (Linux).
    pub code: Option<i32>,
    /// OS đã chặn hoặc kill process vì vượt `ResourceLimits::memory_bytes`.
    pub memory_limit_hit: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("ProcessRuntime phải được tạo bên trong runtime tokio")]
    NoTokio,
    #[error("không chạy được `{program}`: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("process đã thoát")]
    Exited,
    /// Mỗi lệnh console là một dòng; ký tự xuống dòng sẽ biến nó thành nhiều lệnh.
    #[error("lệnh console không được chứa ký tự xuống dòng")]
    MultiLine,
    /// Server chưa đọc stdin (đang nạp mod, hay treo) và hàng lệnh chờ đã đầy.
    #[error("server chưa đọc các lệnh trước, hàng lệnh chờ đã đầy")]
    StdinFull,
}

impl RuntimeError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::NoTokio | Self::Spawn { .. } => ErrorCode::Internal,
            Self::Exited | Self::StdinFull => ErrorCode::ServerBusy,
            Self::MultiLine => ErrorCode::InvalidInput,
        }
    }
}

/// Phải tạo trong runtime tokio và giữ suốt vòng đời controller: drop nó là dừng luồng
/// spawner, kéo theo mọi process server.
#[derive(Debug)]
pub struct ProcessRuntime {
    spawner: spawner::Spawner,
    handle: Handle,
}

/// Process đang chạy. `output` đóng sau khi đã đọc hết log; `exit` báo khi process thoát.
#[derive(Debug)]
pub struct RunningProcess {
    pub pid: Option<u32>,
    pub control: ProcessControl,
    /// Tối đa [`OUTPUT_QUEUE`] dòng chờ đọc; đọc chậm hơn server in thì dòng thừa bị bỏ.
    pub output: mpsc::Receiver<OutputLine>,
    pub exit: oneshot::Receiver<Exit>,
}

/// Gửi lệnh và kill. Clone thoải mái; process vẫn chạy khi mọi bản clone bị drop.
#[derive(Debug, Clone)]
pub struct ProcessControl {
    stdin: mpsc::Sender<String>,
    kill: mpsc::Sender<()>,
}

impl ProcessRuntime {
    pub fn new() -> Result<Self, RuntimeError> {
        let handle = Handle::try_current().map_err(|_| RuntimeError::NoTokio)?;
        let spawner =
            spawner::Spawner::start(handle.clone()).map_err(|source| RuntimeError::Spawn {
                program: "mh-spawner".to_owned(),
                source,
            })?;
        Ok(Self { spawner, handle })
    }

    /// `ProcessSpec::limits` có được OS áp thật không: luôn có trên Windows; trên Linux cần cgroup
    /// v2 được giao quyền (spec P3, mục 3.8).
    pub fn limits_enforced(&self) -> bool {
        self.spawner.limits_enforced()
    }

    pub async fn spawn(&self, spec: ProcessSpec) -> Result<RunningProcess, RuntimeError> {
        let program = spec.program.display().to_string();
        let (mut child, guard) = self
            .spawner
            .spawn(spec)
            .await
            .map_err(|source| RuntimeError::Spawn { program, source })?;
        let pid = child.id();
        let (out_tx, output) = mpsc::channel(OUTPUT_QUEUE);
        // Các task đọc/ghi chạy trên runtime của controller, không phải runtime của người gọi:
        // người gọi có thể là một luồng sắp thoát.
        let mut readers = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            let reader = read_lines(stdout, ConsoleStream::Stdout, out_tx.clone());
            readers.push(self.handle.spawn(reader));
        }
        if let Some(stderr) = child.stderr.take() {
            readers.push(
                self.handle
                    .spawn(read_lines(stderr, ConsoleStream::Stderr, out_tx)),
            );
        }
        let (stdin_tx, stdin_rx) = mpsc::channel(64);
        if let Some(stdin) = child.stdin.take() {
            self.handle.spawn(write_lines(stdin, stdin_rx));
        }
        let (kill_tx, kill_rx) = mpsc::channel(1);
        let (exit_tx, exit) = oneshot::channel();
        self.handle
            .spawn(watch(child, guard, kill_rx, readers, exit_tx));
        Ok(RunningProcess {
            pid,
            control: ProcessControl {
                stdin: stdin_tx,
                kill: kill_tx,
            },
            output,
            exit,
        })
    }
}

impl ProcessControl {
    /// Gửi một dòng vào stdin, kèm `\n`. Không bao giờ chờ: server không đọc stdin (Forge đang nạp
    /// mod, hay đang treo) thì pipe và hàng lệnh đầy, và người gọi (actor của server) mà chờ ở đây thì
    /// cả nút Buộc dừng cũng không tới được.
    pub async fn send_line(&self, line: &str) -> Result<(), RuntimeError> {
        if line.contains(['\n', '\r']) {
            return Err(RuntimeError::MultiLine);
        }
        self.stdin
            .try_send(line.to_owned())
            .map_err(|err| match err {
                TrySendError::Full(_) => RuntimeError::StdinFull,
                TrySendError::Closed(_) => RuntimeError::Exited,
            })
    }

    /// Kill ngay, không chờ server lưu world. Gọi nhiều lần không sao.
    pub fn kill(&self) {
        let _ = self.kill.try_send(());
    }
}

async fn read_lines(
    pipe: impl AsyncRead + Unpin,
    stream: ConsoleStream,
    tx: mpsc::Sender<OutputLine>,
) {
    let mut reader = BufReader::new(pipe);
    let mut out = LineSink::new(stream, tx);
    loop {
        let chunk = match reader.fill_buf().await {
            Ok(chunk) => chunk,
            Err(e) => {
                tracing::debug!(error = %e, "ngừng đọc pipe của process");
                break;
            }
        };
        if chunk.is_empty() {
            out.finish();
            break;
        }
        let newline = chunk.iter().position(|&b| b == b'\n');
        let taken = newline.map_or(chunk.len(), |i| i + 1);
        out.extend(&chunk[..newline.unwrap_or(taken)]);
        reader.consume(taken);
        if newline.is_some() {
            out.emit();
        }
    }
}

/// Gom một dòng (có trần độ dài) rồi đẩy vào hàng đợi mà không bao giờ chờ.
struct LineSink {
    stream: ConsoleStream,
    tx: mpsc::Sender<OutputLine>,
    line: Vec<u8>,
    truncated: bool,
    dropped: u64,
}

impl LineSink {
    fn new(stream: ConsoleStream, tx: mpsc::Sender<OutputLine>) -> Self {
        Self {
            stream,
            tx,
            line: Vec::new(),
            truncated: false,
            dropped: 0,
        }
    }

    fn extend(&mut self, bytes: &[u8]) {
        let room = MAX_LINE_BYTES.saturating_sub(self.line.len());
        if bytes.len() > room {
            self.truncated = true;
        }
        self.line.extend_from_slice(&bytes[..bytes.len().min(room)]);
    }

    /// Dòng cuối không có ký tự xuống dòng vẫn được giữ.
    fn finish(&mut self) {
        if !self.line.is_empty() || self.truncated {
            self.emit();
        }
    }

    fn emit(&mut self) {
        let raw = self.line.strip_suffix(b"\r").unwrap_or(&self.line);
        let mut text = String::from_utf8_lossy(raw).into_owned();
        if self.truncated {
            text.push_str(" …[MineHost: dòng quá dài, đã cắt]");
        }
        self.line.clear();
        self.truncated = false;
        if self.dropped > 0 {
            let note = format!(
                "[MineHost] đã bỏ {} dòng log vì server in nhanh hơn MineHost kịp xử lý",
                self.dropped
            );
            if !self.send(note) {
                self.dropped += 1;
                return;
            }
            self.dropped = 0;
        }
        if !self.send(text) {
            self.dropped += 1;
        }
    }

    /// `false` khi hàng đợi đầy. Không ai nghe thì coi như đã gửi: vẫn đọc tiếp để pipe không đầy.
    fn send(&self, text: String) -> bool {
        let line = OutputLine {
            stream: self.stream,
            text,
        };
        !matches!(self.tx.try_send(line), Err(TrySendError::Full(_)))
    }
}

async fn write_lines(mut stdin: ChildStdin, mut rx: mpsc::Receiver<String>) {
    while let Some(line) = rx.recv().await {
        let data = format!("{line}\n");
        if stdin.write_all(data.as_bytes()).await.is_err() || stdin.flush().await.is_err() {
            break;
        }
    }
}

/// `guard` sống tới khi process thoát: trên Windows nó giữ Job Object riêng của process.
async fn watch(
    mut child: Child,
    guard: os::ProcessGuard,
    mut kill_rx: mpsc::Receiver<()>,
    readers: Vec<JoinHandle<()>>,
    exit_tx: oneshot::Sender<Exit>,
) {
    let status = tokio::select! {
        status = child.wait() => status,
        Some(()) = kill_rx.recv() => {
            tracing::warn!(pid = child.id(), "kill process server");
            guard.kill_tree();
            let _ = child.start_kill();
            child.wait().await
        }
    };
    // Process cháu còn sống sau Java vẫn giữ cổng, file trong world và pipe log: dọn luôn.
    guard.kill_tree();
    let _ = tokio::time::timeout(DRAIN_TIMEOUT, async {
        for reader in readers {
            let _ = reader.await;
        }
    })
    .await;
    let code = match status {
        Ok(status) => status.code(),
        Err(e) => {
            tracing::error!(error = %e, "không đọc được trạng thái thoát của process");
            None
        }
    };
    let memory_limit_hit = guard.memory_limit_hit();
    if memory_limit_hit {
        tracing::warn!(?code, "process server đã chạm giới hạn RAM");
    }
    let _ = exit_tx.send(Exit {
        code,
        memory_limit_hit,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sink(capacity: usize) -> (LineSink, mpsc::Receiver<OutputLine>) {
        let (tx, rx) = mpsc::channel(capacity);
        (LineSink::new(ConsoleStream::Stdout, tx), rx)
    }

    #[tokio::test]
    async fn long_line_is_cut_and_next_line_is_intact() {
        let (tx, mut rx) = mpsc::channel(8);
        let mut input = vec![b'x'; MAX_LINE_BYTES * 3];
        input.extend_from_slice(b"\r\nsau\n");
        read_lines(input.as_slice(), ConsoleStream::Stdout, tx).await;
        let first = rx.recv().await.unwrap().text;
        assert!(
            first.starts_with(&"x".repeat(MAX_LINE_BYTES)),
            "{}",
            first.len()
        );
        assert!(first.ends_with("đã cắt]"));
        assert_eq!(rx.recv().await.unwrap().text, "sau");
        assert!(rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn last_line_without_newline_is_kept() {
        let (tx, mut rx) = mpsc::channel(8);
        read_lines(b"a\nb".as_slice(), ConsoleStream::Stderr, tx).await;
        assert_eq!(rx.recv().await.unwrap().text, "a");
        let last = rx.recv().await.unwrap();
        assert_eq!(
            (last.stream, last.text.as_str()),
            (ConsoleStream::Stderr, "b")
        );
    }

    #[test]
    fn full_queue_drops_lines_then_reports_how_many() {
        let (mut out, mut rx) = sink(2);
        for text in ["a", "b", "c", "d"] {
            out.extend(text.as_bytes());
            out.emit();
        }
        let texts = |rx: &mut mpsc::Receiver<OutputLine>| {
            std::iter::from_fn(|| rx.try_recv().ok().map(|l| l.text)).collect::<Vec<_>>()
        };
        assert_eq!(texts(&mut rx), ["a", "b"]);
        out.extend(b"e");
        out.emit();
        let after = texts(&mut rx);
        assert!(after[0].contains("đã bỏ 2 dòng"), "{after:?}");
        assert_eq!(after[1], "e");
    }
}
