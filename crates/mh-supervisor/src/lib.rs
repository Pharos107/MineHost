//! Một actor cho mỗi server (spec P1, mục 5.6): vòng đời, console, dừng an toàn, tự khởi động
//! lại khi crash.
//!
//! Supervisor không biết EULA, `server.properties` hay Java ở đâu: các việc trước khi bật do
//! [`Launcher`] (cài ở `mh-app`) làm, vì theo bảng crate `mh-supervisor` chỉ được phụ thuộc
//! `mh-runtime` và `mh-core`.

mod actor;
mod console;

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use mh_core::{ConsoleLine, CrashHint, ErrorCode, ServerState};
use mh_runtime::{ProcessRuntime, ProcessSpec, RuntimeError};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

pub use console::Console;

/// Việc phải làm trước mỗi lần bật, kể cả lần tự khởi động lại sau crash.
pub trait Launcher: Send + Sync + 'static {
    /// Kiểm tra EULA, ghi `server-port`, kiểm tra cổng, bảo đảm có Java; trả lệnh chạy.
    fn prepare(&self) -> impl Future<Output = Result<ProcessSpec, LaunchError>> + Send;

    /// Đọc lại mỗi lần crash, vì người dùng có thể đổi cài đặt khi server đang chạy.
    fn restart_on_crash(&self) -> bool;

    /// Bật/tắt dấu `.running` trên đĩa.
    fn set_running_marker(&self, running: bool);
}

/// Lỗi từ [`Launcher::prepare`], đi thẳng tới client.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{code}: {detail}")]
pub struct LaunchError {
    pub code: ErrorCode,
    /// Tham số cho bản dịch, ví dụ `{ "port": 25565, "suggested_port": 25566 }`.
    pub params: serde_json::Value,
    /// Chỉ để ghi log.
    pub detail: String,
}

impl LaunchError {
    pub fn new(code: ErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            params: serde_json::Value::Object(Default::default()),
            detail: detail.into(),
        }
    }

    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = params;
        self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    /// Thao tác không hợp lệ với trạng thái hiện tại.
    #[error("server đang ở trạng thái {0:?}, không làm được thao tác này")]
    Busy(ServerState),
    #[error(transparent)]
    Launch(LaunchError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    /// Actor đã dừng (controller đang tắt).
    #[error("supervisor đã dừng")]
    Gone,
}

impl SupervisorError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Busy(_) => ErrorCode::ServerBusy,
            Self::Launch(e) => e.code,
            Self::Runtime(e) => e.code(),
            Self::Gone => ErrorCode::Internal,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// Chưa thấy dòng `Done` sau chừng này thì cảnh báo (không kill).
    pub slow_start_after: Duration,
    /// Gửi `stop` rồi chờ chừng này, quá hạn thì kill.
    pub stop_timeout: Duration,
    /// Thời gian chờ trước lần tự khởi động lại thứ 1, 2, 3…; hết danh sách thì dừng ở `Crashed`.
    pub restart_delays: Vec<Duration>,
    /// Chỉ đếm các lần crash trong khoảng này.
    pub restart_window: Duration,
    pub console_lines: usize,
    /// `servers/<id>/logs/`; `None` thì không ghi file.
    pub logs_dir: Option<PathBuf>,
    pub keep_log_days: i64,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            slow_start_after: Duration::from_secs(5 * 60),
            stop_timeout: Duration::from_secs(60),
            restart_delays: [5, 15, 30].map(Duration::from_secs).to_vec(),
            restart_window: Duration::from_secs(10 * 60),
            console_lines: 5000,
            logs_dir: None,
            keep_log_days: 14,
        }
    }
}

/// Ảnh chụp trạng thái, đọc không cần đi qua actor.
#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    pub state: ServerState,
    pub pid: Option<u32>,
    /// Có khi `state` là `Crashed`.
    pub crash: Option<Arc<CrashInfo>>,
    /// Đang chờ tự khởi động lại sau crash.
    pub restart_pending: bool,
    /// Đã quá `slow_start_after` mà chưa thấy dòng `Done`.
    pub slow_start: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashInfo {
    pub hint: Option<CrashHint>,
    pub exit_code: Option<i32>,
    /// 200 dòng log cuối.
    pub last_lines: Vec<String>,
}

/// Sự kiện phát cho mọi người nghe (WebSocket, tooltip khay…).
#[derive(Debug, Clone, PartialEq)]
pub enum SupervisorEvent {
    Status(Status),
    Console(ConsoleLine),
}

pub(crate) enum Command {
    Start(oneshot::Sender<Result<(), SupervisorError>>),
    Stop(oneshot::Sender<Result<(), SupervisorError>>),
    Restart(oneshot::Sender<Result<(), SupervisorError>>),
    Kill(oneshot::Sender<Result<(), SupervisorError>>),
    Send(String, oneshot::Sender<Result<(), SupervisorError>>),
}

/// Điều khiển một server. Clone thoải mái; actor dừng khi mọi bản clone bị drop.
#[derive(Debug, Clone)]
pub struct SupervisorHandle {
    tx: mpsc::Sender<Command>,
    status: watch::Receiver<Status>,
    console: Arc<Console>,
    events: broadcast::Sender<SupervisorEvent>,
}

/// Tạo actor cho một server đã cài xong (trạng thái đầu là `Stopped`).
pub fn spawn<L: Launcher>(
    config: SupervisorConfig,
    runtime: Arc<ProcessRuntime>,
    launcher: L,
) -> SupervisorHandle {
    let console = Arc::new(Console::new(
        config.console_lines,
        config.logs_dir.clone(),
        config.keep_log_days,
    ));
    let (tx, rx) = mpsc::channel(32);
    let (events, _) = broadcast::channel(1024);
    let (status_tx, status) = watch::channel(Status {
        state: ServerState::Stopped,
        pid: None,
        crash: None,
        restart_pending: false,
        slow_start: false,
    });
    let actor = actor::Actor::new(
        config,
        runtime,
        launcher,
        Arc::clone(&console),
        status_tx,
        events.clone(),
    );
    tokio::spawn(actor.run(rx));
    SupervisorHandle {
        tx,
        status,
        console,
        events,
    }
}

impl SupervisorHandle {
    pub async fn start(&self) -> Result<(), SupervisorError> {
        self.call(Command::Start).await
    }

    /// Gửi `stop` rồi trả về ngay; theo dõi `status` để biết khi nào đã dừng.
    pub async fn stop(&self) -> Result<(), SupervisorError> {
        self.call(Command::Stop).await
    }

    pub async fn restart(&self) -> Result<(), SupervisorError> {
        self.call(Command::Restart).await
    }

    pub async fn kill(&self) -> Result<(), SupervisorError> {
        self.call(Command::Kill).await
    }

    /// Gửi nguyên văn một lệnh console tới stdin của server.
    pub async fn send(&self, line: impl Into<String>) -> Result<(), SupervisorError> {
        let line = line.into();
        self.call(|reply| Command::Send(line, reply)).await
    }

    pub fn status(&self) -> Status {
        self.status.borrow().clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SupervisorEvent> {
        self.events.subscribe()
    }

    pub fn console(&self) -> &Console {
        &self.console
    }

    /// Chờ tới khi `pred` đúng với trạng thái, tối đa `timeout`. Trả trạng thái cuối cùng thấy.
    pub async fn wait_for(&self, timeout: Duration, pred: impl Fn(&Status) -> bool) -> Status {
        let mut rx = self.status.clone();
        let _ = tokio::time::timeout(timeout, rx.wait_for(|s| pred(s))).await;
        self.status()
    }

    /// Dừng an toàn và chờ xong, dùng khi thoát MineHost. Server không chạy thì trả về ngay.
    pub async fn stop_and_wait(&self, timeout: Duration) -> Status {
        let now = self.status();
        if now.state.has_process() || now.restart_pending {
            // Đang `Stopping` thì `stop` báo bận, vẫn chờ như thường.
            let _ = self.stop().await;
        }
        let done = |s: &Status| !s.state.has_process() && !s.restart_pending;
        self.wait_for(timeout, done).await
    }

    async fn call(
        &self,
        make: impl FnOnce(oneshot::Sender<Result<(), SupervisorError>>) -> Command,
    ) -> Result<(), SupervisorError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(make(reply))
            .await
            .map_err(|_| SupervisorError::Gone)?;
        rx.await.map_err(|_| SupervisorError::Gone)?
    }
}
