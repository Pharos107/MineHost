//! Vòng lặp của actor. Mọi thay đổi trạng thái đi qua `mh_core::transition`.

use std::collections::VecDeque;
use std::sync::Arc;

use mh_core::state::is_done_line;
use mh_core::{ConsoleStream, ServerEvent, ServerState, diagnose, parse_log_line, transition};
use mh_runtime::{Exit, OutputLine, ProcessControl, ProcessRuntime, RunningProcess};
use time::OffsetDateTime;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::Instant;

use crate::{
    Command, Console, CrashInfo, Launcher, Status, SupervisorConfig, SupervisorError,
    SupervisorEvent,
};

/// Số dòng log cuối giữ lại khi crash (spec P1, mục 4.2).
const CRASH_TAIL: usize = 200;

type Reply = oneshot::Sender<Result<(), SupervisorError>>;

struct Live {
    control: ProcessControl,
    output: Option<mpsc::UnboundedReceiver<OutputLine>>,
    exit: oneshot::Receiver<Exit>,
}

enum Wake {
    Command(Option<Command>),
    Line(OutputLine),
    OutputClosed,
    Exited(Option<i32>),
    StopTimeout,
    RestartDue,
    SlowStart,
}

pub(crate) struct Actor<L> {
    config: SupervisorConfig,
    runtime: Arc<ProcessRuntime>,
    launcher: L,
    console: Arc<Console>,
    status_tx: watch::Sender<Status>,
    events: broadcast::Sender<SupervisorEvent>,
    status: Status,
    live: Option<Live>,
    /// Người dùng đã yêu cầu dừng: process thoát lúc này không phải crash.
    restart_after_stop: bool,
    stop_deadline: Option<Instant>,
    restart_at: Option<Instant>,
    slow_start_at: Option<Instant>,
    crashes: VecDeque<Instant>,
}

impl<L: Launcher> Actor<L> {
    pub(crate) fn new(
        config: SupervisorConfig,
        runtime: Arc<ProcessRuntime>,
        launcher: L,
        console: Arc<Console>,
        status_tx: watch::Sender<Status>,
        events: broadcast::Sender<SupervisorEvent>,
    ) -> Self {
        let status = status_tx.borrow().clone();
        Self {
            config,
            runtime,
            launcher,
            console,
            status_tx,
            events,
            status,
            live: None,
            restart_after_stop: false,
            stop_deadline: None,
            restart_at: None,
            slow_start_at: None,
            crashes: VecDeque::new(),
        }
    }

    pub(crate) async fn run(mut self, mut rx: mpsc::Receiver<Command>) {
        loop {
            match self.next(&mut rx).await {
                Wake::Command(None) => break,
                Wake::Command(Some(cmd)) => self.on_command(cmd).await,
                Wake::Line(line) => self.on_line(line).await,
                Wake::OutputClosed => {
                    if let Some(live) = self.live.as_mut() {
                        live.output = None;
                    }
                }
                Wake::Exited(code) => self.on_exit(code).await,
                Wake::StopTimeout => self.on_stop_timeout(),
                Wake::RestartDue => self.on_restart_due().await,
                Wake::SlowStart => self.on_slow_start(),
            }
        }
        // Mọi handle đã bị drop: controller đang tắt. Không để process sống tiếp không ai quản.
        if let Some(live) = self.live.take() {
            live.control.kill();
        }
    }

    async fn next(&mut self, rx: &mut mpsc::Receiver<Command>) -> Wake {
        let (stop, restart, slow) = (self.stop_deadline, self.restart_at, self.slow_start_at);
        let live = self.live.as_mut();
        let (output, exit) = match live {
            Some(Live { output, exit, .. }) => (output.as_mut(), Some(exit)),
            None => (None, None),
        };
        tokio::select! {
            biased;
            cmd = rx.recv() => Wake::Command(cmd),
            line = recv_line(output) => line.map_or(Wake::OutputClosed, Wake::Line),
            exit = wait_exit(exit) => Wake::Exited(exit),
            () = sleep_until(stop) => Wake::StopTimeout,
            () = sleep_until(restart) => Wake::RestartDue,
            () = sleep_until(slow) => Wake::SlowStart,
        }
    }

    async fn on_command(&mut self, cmd: Command) {
        match cmd {
            Command::Start(reply) => {
                // Người dùng tự bấm Bật: bỏ đếm crash cũ và lịch tự khởi động lại.
                self.crashes.clear();
                self.restart_at = None;
                let result = self.start().await;
                respond(reply, result);
            }
            Command::Stop(reply) => respond(reply, self.stop().await),
            Command::Kill(reply) => respond(reply, self.kill()),
            Command::Restart(reply) => respond(reply, self.restart().await),
            Command::Send(line, reply) => respond(reply, self.send(line).await),
        }
    }

    async fn start(&mut self) -> Result<(), SupervisorError> {
        let next = self.check(ServerEvent::Start)?;
        let spec = self
            .launcher
            .prepare()
            .await
            .map_err(SupervisorError::Launch)?;
        let RunningProcess {
            pid,
            control,
            output,
            exit,
        } = self.runtime.spawn(spec).await?;
        self.launcher.set_running_marker(true);
        self.live = Some(Live {
            control,
            output: Some(output),
            exit,
        });
        self.slow_start_at = Some(Instant::now() + self.config.slow_start_after);
        self.status.pid = pid;
        self.status.crash = None;
        self.status.slow_start = false;
        self.set_state(next);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), SupervisorError> {
        // Đang chờ tự khởi động lại sau crash: "Tắt" nghĩa là hủy lịch đó.
        if self.status.state == ServerState::Crashed && self.restart_at.is_some() {
            self.restart_at = None;
            self.launcher.set_running_marker(false);
            self.publish();
            return Ok(());
        }
        let next = self.check(ServerEvent::Stop)?;
        if let Some(live) = &self.live {
            let control = live.control.clone();
            self.echo("stop".to_owned());
            // Stdin đã đóng thì process sắp thoát; hạn chót bên dưới vẫn bảo đảm nó dừng.
            let _ = control.send_line("stop").await;
        }
        self.stop_deadline = Some(Instant::now() + self.config.stop_timeout);
        self.set_state(next);
        Ok(())
    }

    fn kill(&mut self) -> Result<(), SupervisorError> {
        let next = self.check(ServerEvent::Kill)?;
        if let Some(live) = &self.live {
            live.control.kill();
        }
        self.stop_deadline = None;
        self.set_state(next);
        Ok(())
    }

    async fn restart(&mut self) -> Result<(), SupervisorError> {
        match self.status.state {
            ServerState::Starting | ServerState::Running => {
                self.stop().await?;
                self.restart_after_stop = true;
                Ok(())
            }
            ServerState::Stopped | ServerState::Crashed => {
                self.restart_at = None;
                self.start().await
            }
            other => Err(SupervisorError::Busy(other)),
        }
    }

    async fn send(&mut self, line: String) -> Result<(), SupervisorError> {
        let state = self.status.state;
        let Some(live) = self.live.as_ref().filter(|_| state.has_process()) else {
            return Err(SupervisorError::Busy(state));
        };
        let control = live.control.clone();
        control.send_line(&line).await?;
        self.echo(line);
        Ok(())
    }

    /// Ghi lệnh đã gửi vào console, để lịch sử hiện đủ cả lệnh lẫn kết quả.
    fn echo(&self, line: String) {
        let echoed = self.console.push(ConsoleStream::Stdin, line);
        let _ = self.events.send(SupervisorEvent::Console(echoed));
    }

    async fn on_line(&mut self, line: OutputLine) {
        let ready = self.status.state == ServerState::Starting && is_done_line(&line.text);
        // Ghi console trước khi phát sự kiện, để người nghe đọc bộ đệm là đã có dòng đó.
        let stream = line.stream;
        let pushed = self.console.push(stream, line.text);
        self.note_players(stream, &pushed.text);
        let _ = self.events.send(SupervisorEvent::Console(pushed));
        if ready && let Ok(next) = transition(self.status.state, ServerEvent::Ready) {
            self.slow_start_at = None;
            self.status.slow_start = false;
            self.set_state(next);
            // Một lần `list` ngay khi sang Running, để danh sách online không lệch (spec 3.2).
            self.sync_online_players().await;
        }
    }

    /// Stderr không phải log server. Dòng không phải join/leave/`list` thì bỏ qua.
    fn note_players(&self, stream: ConsoleStream, text: &str) {
        if stream != ConsoleStream::Stdout {
            return;
        }
        let Some(event) = parse_log_line(text) else {
            return;
        };
        let _ = self.events.send(SupervisorEvent::Players(event));
    }

    async fn sync_online_players(&self) {
        let Some(live) = self.live.as_ref() else {
            return;
        };
        let control = live.control.clone();
        // Không echo: đây là lệnh của MineHost, không phải lịch sử người dùng gõ.
        if let Err(error) = control.send_line("list").await {
            tracing::warn!(error = %error, "không gửi được lệnh list để đồng bộ người chơi");
        }
    }

    async fn on_exit(&mut self, code: Option<i32>) {
        // Đọc nốt các dòng còn trong kênh, để thông tin crash có đủ log cuối.
        if let Some(mut live) = self.live.take()
            && let Some(output) = live.output.as_mut()
        {
            while let Ok(line) = output.try_recv() {
                self.on_line(line).await;
            }
        }
        self.console.close_log();
        self.stop_deadline = None;
        self.slow_start_at = None;
        self.status.pid = None;
        self.status.running_since = None;
        self.status.slow_start = false;
        let Ok(next) = transition(self.status.state, ServerEvent::ProcessExited) else {
            tracing::error!(state = ?self.status.state, "process thoát ở trạng thái không ngờ tới");
            return;
        };
        if next == ServerState::Stopped {
            self.launcher.set_running_marker(false);
            self.set_state(next);
            if std::mem::take(&mut self.restart_after_stop)
                && let Err(e) = self.start().await
            {
                tracing::warn!(error = %e, "không bật lại được server sau khi khởi động lại");
            }
            return;
        }
        self.on_crash(code);
        self.set_state(next);
    }

    fn on_crash(&mut self, code: Option<i32>) {
        let last_lines = self.console.tail(CRASH_TAIL);
        let hint = diagnose(&last_lines);
        tracing::warn!(?code, ?hint, "server crash");
        self.status.crash = Some(Arc::new(CrashInfo {
            hint,
            exit_code: code,
            last_lines,
        }));
        self.restart_after_stop = false;
        let now = Instant::now();
        let window = self.config.restart_window;
        self.crashes.retain(|t| now.duration_since(*t) < window);
        let delay = self.config.restart_delays.get(self.crashes.len()).copied();
        match delay {
            Some(delay) if self.launcher.restart_on_crash() => {
                self.crashes.push_back(now);
                self.restart_at = Some(now + delay);
            }
            _ => self.launcher.set_running_marker(false),
        }
    }

    fn on_stop_timeout(&mut self) {
        self.stop_deadline = None;
        if let Some(live) = &self.live {
            tracing::warn!("server không dừng kịp sau lệnh stop, buộc dừng");
            live.control.kill();
        }
    }

    async fn on_restart_due(&mut self) {
        self.restart_at = None;
        if let Err(e) = self.start().await {
            tracing::warn!(error = %e, "tự khởi động lại thất bại");
            self.launcher.set_running_marker(false);
            self.publish();
        }
    }

    fn on_slow_start(&mut self) {
        self.slow_start_at = None;
        tracing::warn!("server bật quá lâu mà chưa sẵn sàng");
        self.status.slow_start = true;
        self.publish();
    }

    fn check(&self, event: ServerEvent) -> Result<ServerState, SupervisorError> {
        transition(self.status.state, event).map_err(|_| SupervisorError::Busy(self.status.state))
    }

    fn set_state(&mut self, state: ServerState) {
        if state != self.status.state {
            tracing::info!(from = ?self.status.state, to = ?state, "đổi trạng thái server");
        }
        // "Thời gian chạy" tính từ lúc sẵn sàng, không tính khoảng Starting.
        if state == ServerState::Running && self.status.state != ServerState::Running {
            self.status.running_since = Some(OffsetDateTime::now_utc());
        }
        self.status.state = state;
        self.publish();
    }

    fn publish(&mut self) {
        self.status.restart_pending = self.restart_at.is_some();
        self.status_tx.send_replace(self.status.clone());
        let _ = self
            .events
            .send(SupervisorEvent::Status(self.status.clone()));
    }
}

fn respond(reply: Reply, result: Result<(), SupervisorError>) {
    let _ = reply.send(result);
}

async fn recv_line(output: Option<&mut mpsc::UnboundedReceiver<OutputLine>>) -> Option<OutputLine> {
    match output {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    }
}

async fn wait_exit(exit: Option<&mut oneshot::Receiver<Exit>>) -> Option<i32> {
    match exit {
        // Kênh đóng mà không có kết quả nghĩa là task theo dõi đã mất: coi như process đã thoát.
        Some(rx) => rx.await.ok().and_then(|e| e.code),
        None => std::future::pending().await,
    }
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}
