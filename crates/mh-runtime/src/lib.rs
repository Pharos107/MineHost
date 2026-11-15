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
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

pub use complete::Completion;
pub use launch::{ENCODING_FLAGS, JavaLaunch};

/// Sau khi process thoát, chờ tối đa chừng này để đọc nốt log còn trong pipe. Process cháu
/// có thể giữ pipe mở mãi, nên không chờ vô hạn.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

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
}

impl RuntimeError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::NoTokio | Self::Spawn { .. } => ErrorCode::Internal,
            Self::Exited => ErrorCode::ServerBusy,
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
    pub output: mpsc::UnboundedReceiver<OutputLine>,
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
        let (out_tx, output) = mpsc::unbounded_channel();
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
    /// Gửi một dòng vào stdin, kèm `\n`.
    pub async fn send_line(&self, line: &str) -> Result<(), RuntimeError> {
        if line.contains(['\n', '\r']) {
            return Err(RuntimeError::MultiLine);
        }
        self.stdin
            .send(line.to_owned())
            .await
            .map_err(|_| RuntimeError::Exited)
    }

    /// Kill ngay, không chờ server lưu world. Gọi nhiều lần không sao.
    pub fn kill(&self) {
        let _ = self.kill.try_send(());
    }
}

async fn read_lines(
    pipe: impl AsyncRead + Unpin,
    stream: ConsoleStream,
    tx: mpsc::UnboundedSender<OutputLine>,
) {
    let mut reader = BufReader::new(pipe);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) => break,
            Ok(_) => {
                let line = buf.strip_suffix(b"\n").unwrap_or(&buf);
                let line = line.strip_suffix(b"\r").unwrap_or(line);
                let text = String::from_utf8_lossy(line).into_owned();
                // Không ai nghe thì vẫn đọc tiếp, để pipe không đầy làm server bị treo.
                let _ = tx.send(OutputLine { stream, text });
            }
            Err(e) => {
                tracing::debug!(error = %e, "ngừng đọc pipe của process");
                break;
            }
        }
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
            let _ = child.start_kill();
            child.wait().await
        }
    };
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
