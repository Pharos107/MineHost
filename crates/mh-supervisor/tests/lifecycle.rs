//! Test vòng đời supervisor với `fake-mc`; không cần Java. Các thời hạn được rút ngắn.

// Hàm trợ giúp ngoài `#[test]` không được clippy coi là code test.
#![allow(clippy::unwrap_used)]

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use mh_core::{ConsoleStream, CrashHint, ErrorCode, PlayerLogEvent, PlayerName, ServerState};
use mh_runtime::{ProcessRuntime, ProcessSpec};
use mh_supervisor::{
    LaunchError, Launcher, Status, SupervisorConfig, SupervisorEvent, SupervisorHandle,
};
use tokio::sync::broadcast;

const WAIT: Duration = Duration::from_secs(10);

#[derive(Clone, Default)]
struct Probe {
    prepares: Arc<AtomicUsize>,
    marker: Arc<AtomicBool>,
}

struct FakeLauncher {
    args: Vec<String>,
    restart_on_crash: bool,
    fail: Option<ErrorCode>,
    probe: Probe,
}

impl Launcher for FakeLauncher {
    fn prepare(&self) -> impl Future<Output = Result<ProcessSpec, LaunchError>> + Send {
        self.probe.prepares.fetch_add(1, Ordering::SeqCst);
        let result = match self.fail {
            Some(code) => Err(LaunchError::new(code, "test")),
            None => Ok(ProcessSpec {
                program: PathBuf::from(env!("CARGO_BIN_EXE_supervisor-fake-mc")),
                args: self.args.clone(),
                work_dir: std::env::temp_dir(),
            }),
        };
        async move { result }
    }

    fn restart_on_crash(&self) -> bool {
        self.restart_on_crash
    }

    fn set_running_marker(&self, running: bool) {
        self.probe.marker.store(running, Ordering::SeqCst);
    }
}

fn config() -> SupervisorConfig {
    SupervisorConfig {
        slow_start_after: Duration::from_millis(300),
        stop_timeout: Duration::from_millis(300),
        restart_delays: vec![Duration::from_millis(20); 3],
        ..SupervisorConfig::default()
    }
}

struct Setup {
    args: &'static [&'static str],
    restart_on_crash: bool,
    fail: Option<ErrorCode>,
    config: SupervisorConfig,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            args: &[],
            restart_on_crash: true,
            fail: None,
            config: config(),
        }
    }
}

fn supervisor(setup: Setup) -> (SupervisorHandle, Probe) {
    let runtime = Arc::new(ProcessRuntime::new().unwrap());
    let probe = Probe::default();
    let launcher = FakeLauncher {
        args: setup.args.iter().map(|a| (*a).to_owned()).collect(),
        restart_on_crash: setup.restart_on_crash,
        fail: setup.fail,
        probe: probe.clone(),
    };
    (mh_supervisor::spawn(setup.config, runtime, launcher), probe)
}

async fn wait_state(sup: &SupervisorHandle, state: ServerState) -> Status {
    let status = sup.wait_for(WAIT, |s| s.state == state).await;
    assert_eq!(status.state, state, "{status:?}");
    status
}

#[tokio::test]
async fn start_reaches_running_then_stop_reaches_stopped() {
    let (sup, probe) = supervisor(Setup::default());
    sup.start().await.unwrap();
    assert!(probe.marker.load(Ordering::SeqCst));
    let running = wait_state(&sup, ServerState::Running).await;
    assert!(running.pid.is_some());

    sup.stop().await.unwrap();
    let stopped = wait_state(&sup, ServerState::Stopped).await;
    assert_eq!(stopped.pid, None);
    assert!(!probe.marker.load(Ordering::SeqCst));
    let texts: Vec<String> = sup
        .console()
        .after(None)
        .into_iter()
        .map(|l| l.text)
        .collect();
    assert!(
        texts.iter().any(|t| t.contains("Stopping server")),
        "{texts:?}"
    );
}

#[tokio::test]
async fn start_twice_is_busy() {
    let (sup, _) = supervisor(Setup::default());
    sup.start().await.unwrap();
    let err = sup.start().await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::ServerBusy);
    sup.kill().await.unwrap();
}

#[tokio::test]
async fn hung_stop_is_killed_after_timeout() {
    let (sup, probe) = supervisor(Setup {
        args: &["--hang-on-stop"],
        ..Setup::default()
    });
    sup.start().await.unwrap();
    wait_state(&sup, ServerState::Running).await;

    sup.stop().await.unwrap();
    assert_eq!(sup.status().state, ServerState::Stopping);
    // Người dùng yêu cầu dừng nên đây không phải crash, dù process bị kill.
    let stopped = wait_state(&sup, ServerState::Stopped).await;
    assert_eq!(stopped.crash, None);
    assert!(!probe.marker.load(Ordering::SeqCst));
}

#[tokio::test]
async fn four_crashes_stay_crashed() {
    let (sup, probe) = supervisor(Setup {
        args: &["--crash-after-ms", "30"],
        ..Setup::default()
    });
    sup.start().await.unwrap();

    let done = sup
        .wait_for(WAIT, |s| {
            s.state == ServerState::Crashed
                && !s.restart_pending
                && probe.prepares.load(Ordering::SeqCst) == 4
        })
        .await;
    assert_eq!(done.state, ServerState::Crashed);
    // Thêm một chút thời gian để chắc không có lần thứ năm.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(probe.prepares.load(Ordering::SeqCst), 4);
    assert!(!probe.marker.load(Ordering::SeqCst));
    let crash = sup.status().crash.unwrap();
    assert_eq!(crash.hint, Some(CrashHint::NotEnoughMemory));
    assert_eq!(crash.exit_code, Some(1));
    assert!(
        crash
            .last_lines
            .iter()
            .any(|l| l.contains("OutOfMemoryError"))
    );
}

#[tokio::test]
async fn crash_without_restart_policy_stays_crashed() {
    let (sup, probe) = supervisor(Setup {
        args: &["--crash-after-ms", "30"],
        restart_on_crash: false,
        ..Setup::default()
    });
    sup.start().await.unwrap();
    let status = wait_state(&sup, ServerState::Crashed).await;
    assert!(!status.restart_pending);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(probe.prepares.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn stop_while_waiting_to_restart_cancels_it() {
    let mut config = config();
    config.restart_delays = vec![Duration::from_secs(30)];
    let (sup, probe) = supervisor(Setup {
        args: &["--crash-after-ms", "30"],
        config,
        ..Setup::default()
    });
    sup.start().await.unwrap();
    let status = sup.wait_for(WAIT, |s| s.restart_pending).await;
    assert_eq!(status.state, ServerState::Crashed);

    sup.stop().await.unwrap();
    assert!(!sup.status().restart_pending);
    assert!(!probe.marker.load(Ordering::SeqCst));
}

#[tokio::test]
async fn console_records_commands_and_output() {
    let (sup, _) = supervisor(Setup::default());
    let mut events = sup.subscribe();
    sup.start().await.unwrap();
    wait_state(&sup, ServerState::Running).await;

    sup.send("say chào cả nhà").await.unwrap();
    let echoed = tokio::time::timeout(WAIT, async {
        loop {
            if let Ok(mh_supervisor::SupervisorEvent::Console(line)) = events.recv().await
                && line.text.contains("[Server] chào cả nhà")
            {
                return line;
            }
        }
    })
    .await
    .unwrap();

    let lines = sup.console().after(None);
    assert!(lines.windows(2).all(|w| w[1].seq > w[0].seq));
    let cmd = lines
        .iter()
        .find(|l| l.stream == ConsoleStream::Stdin)
        .unwrap();
    assert_eq!(cmd.text, "say chào cả nhà");
    assert!(cmd.seq < echoed.seq);
    assert!(sup.console().after(Some(echoed.seq)).is_empty());
    sup.kill().await.unwrap();
}

#[tokio::test]
async fn send_when_stopped_is_busy() {
    let (sup, _) = supervisor(Setup::default());
    let err = sup.send("say x").await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::ServerBusy);
}

#[tokio::test]
async fn launch_error_keeps_server_stopped() {
    let (sup, probe) = supervisor(Setup {
        fail: Some(ErrorCode::EulaNotAccepted),
        ..Setup::default()
    });
    let err = sup.start().await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::EulaNotAccepted);
    assert_eq!(sup.status().state, ServerState::Stopped);
    assert!(!probe.marker.load(Ordering::SeqCst));
}

#[tokio::test]
async fn slow_start_warns_but_keeps_starting() {
    let (sup, _) = supervisor(Setup {
        args: &["--no-done"],
        ..Setup::default()
    });
    sup.start().await.unwrap();
    let status = sup.wait_for(WAIT, |s| s.slow_start).await;
    assert!(status.slow_start);
    assert_eq!(status.state, ServerState::Starting);
    sup.kill().await.unwrap();
    wait_state(&sup, ServerState::Stopped).await;
}

#[tokio::test]
async fn running_sends_list_and_join_is_a_player_event() {
    let (sup, _) = supervisor(Setup::default());
    let mut events = sup.subscribe();
    sup.start().await.unwrap();
    wait_state(&sup, ServerState::Running).await;

    let listed = next_players(&mut events).await;
    assert_eq!(
        listed,
        PlayerLogEvent::List {
            online: 0,
            max: 20,
            names: vec![],
        }
    );
    let console = sup.console().after(None);
    assert!(
        console.iter().any(|line| {
            line.stream == ConsoleStream::Stdout && line.text.contains("players online:")
        }),
        "server phải trả lời lệnh list"
    );
    assert!(
        !console
            .iter()
            .any(|line| line.stream == ConsoleStream::Stdin && line.text == "list"),
        "lệnh list tự gửi không vào lịch sử lệnh"
    );

    sup.send("fake-join Steve").await.unwrap();
    let joined = next_players(&mut events).await;
    assert_eq!(
        joined,
        PlayerLogEvent::Joined(PlayerName::parse("Steve").unwrap())
    );
    sup.kill().await.unwrap();
}

async fn next_players(events: &mut broadcast::Receiver<SupervisorEvent>) -> PlayerLogEvent {
    tokio::time::timeout(WAIT, async {
        loop {
            if let Ok(SupervisorEvent::Players(event)) = events.recv().await {
                return event;
            }
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn restart_runs_the_server_again() {
    let (sup, probe) = supervisor(Setup::default());
    sup.start().await.unwrap();
    wait_state(&sup, ServerState::Running).await;
    let first_pid = sup.status().pid;

    sup.restart().await.unwrap();
    let status = sup
        .wait_for(WAIT, |s| {
            s.state == ServerState::Running && s.pid != first_pid
        })
        .await;
    assert_eq!(status.state, ServerState::Running);
    assert_eq!(probe.prepares.load(Ordering::SeqCst), 2);
    sup.kill().await.unwrap();
}

#[tokio::test]
async fn stop_and_wait_returns_when_stopped() {
    let (sup, _) = supervisor(Setup::default());
    sup.start().await.unwrap();
    wait_state(&sup, ServerState::Running).await;
    let status = sup.stop_and_wait(WAIT).await;
    assert_eq!(status.state, ServerState::Stopped);
    // Đã dừng rồi thì gọi lại trả về ngay.
    assert_eq!(sup.stop_and_wait(WAIT).await.state, ServerState::Stopped);
}

#[tokio::test]
async fn writes_console_log_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config();
    config.logs_dir = Some(tmp.path().to_path_buf());
    let (sup, _) = supervisor(Setup {
        config,
        ..Setup::default()
    });
    sup.start().await.unwrap();
    wait_state(&sup, ServerState::Running).await;
    sup.stop_and_wait(WAIT).await;

    let file = std::fs::read_dir(tmp.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let text = std::fs::read_to_string(file.path()).unwrap();
    assert!(text.contains("Xin chào thế giới"), "{text}");
    assert!(text.contains("> stop"), "{text}");
}
