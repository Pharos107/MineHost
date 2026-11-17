//! Launcher giả và hàm chờ dùng chung cho test vòng đời supervisor.

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use mh_core::{ErrorCode, ServerState};
use mh_runtime::{ProcessRuntime, ProcessSpec};
use mh_supervisor::{LaunchError, Launcher, Status, SupervisorConfig, SupervisorHandle};

pub const WAIT: Duration = Duration::from_secs(10);

#[derive(Clone, Default)]
pub struct Probe {
    pub prepares: Arc<AtomicUsize>,
    pub marker: Arc<AtomicBool>,
}

pub struct FakeLauncher {
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
                limits: None,
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

pub fn config() -> SupervisorConfig {
    SupervisorConfig {
        slow_start_after: Duration::from_millis(300),
        stop_timeout: Duration::from_millis(300),
        restart_delays: vec![Duration::from_millis(20); 3],
        ..SupervisorConfig::default()
    }
}

pub struct Setup {
    pub args: &'static [&'static str],
    pub restart_on_crash: bool,
    pub fail: Option<ErrorCode>,
    pub config: SupervisorConfig,
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

pub fn supervisor(setup: Setup) -> (SupervisorHandle, Probe) {
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

pub async fn wait_state(sup: &SupervisorHandle, state: ServerState) -> Status {
    let status = sup.wait_for(WAIT, |s| s.state == state).await;
    assert_eq!(status.state, state, "{status:?}");
    status
}

pub fn console_has(sup: &SupervisorHandle, text: &str) -> bool {
    sup.console()
        .after(None)
        .iter()
        .any(|line| line.text.contains(text))
}
