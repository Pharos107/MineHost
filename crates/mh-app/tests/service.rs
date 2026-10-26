//! Test `mh-app` với catalog và Java giả; "Java" là `fake-mc`, nên server bật được mà không cần
//! mạng hay Java thật.

// Hàm trợ giúp ngoài `#[test]` không được clippy coi là code test.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mh_app::sources::{BoxFut, CatalogVersion, ProgressFn, ResolvedServer};
use mh_app::{
    App, AppDeps, AppError, CreateRequest, JavaSource, JobState, ServerCatalog, SettingsPatch,
};
use mh_core::{ErrorCode, GamePort, InstanceSpec, LoaderKind, McVersion, ServerName, ServerState};
use mh_java::JavaRuntime;
use mh_net::{CancellationToken, Checksum};
use mh_platform::DataDirs;
use mh_providers::ServerJar;
use mh_supervisor::SupervisorConfig;
use time::macros::datetime;
use tokio::sync::Notify;

const WAIT: Duration = Duration::from_secs(10);

#[derive(Default)]
struct FakeCatalog {
    /// Có thì việc tải jar dừng lại chờ tới khi bị hủy hoặc được mở.
    gate: Option<Arc<Notify>>,
    fail_resolve: bool,
}

impl ServerCatalog for FakeCatalog {
    fn versions(&self, _: LoaderKind) -> BoxFut<'_, Result<Vec<CatalogVersion>, AppError>> {
        Box::pin(async {
            Ok(vec![CatalogVersion {
                id: McVersion::parse("1.21.4").unwrap(),
                release_time: datetime!(2024-12-03 10:12 UTC),
            }])
        })
    }

    fn resolve<'a>(
        &'a self,
        _: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<ResolvedServer, AppError>> {
        Box::pin(async move {
            if self.fail_resolve {
                return Err(AppError::new(ErrorCode::VersionNotFound, "giả"));
            }
            Ok(ResolvedServer {
                minecraft: minecraft.clone(),
                build: Some(232),
                java_major: 21,
                jar: ServerJar {
                    url: "https://example.invalid/paper.jar".to_owned(),
                    checksum: Checksum::sha256(&"0".repeat(64)).unwrap(),
                    size: 8,
                },
            })
        })
    }

    fn download<'a>(
        &'a self,
        _: &'a ServerJar,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            progress(4, Some(8));
            if let Some(gate) = &self.gate {
                tokio::select! {
                    () = cancel.cancelled() => {
                        return Err(AppError::new(ErrorCode::JobCancelled, "hủy"));
                    }
                    () = gate.notified() => {}
                }
            }
            std::fs::write(dest, b"fake jar").unwrap();
            progress(8, Some(8));
            Ok(())
        })
    }
}

struct FakeJava;

impl JavaSource for FakeJava {
    fn ensure<'a>(
        &'a self,
        major: u16,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<JavaRuntime, AppError>> {
        Box::pin(async move {
            let bin = PathBuf::from(env!("CARGO_BIN_EXE_app-fake-mc"));
            Ok(JavaRuntime {
                major,
                home: bin.parent().unwrap().to_path_buf(),
                java_bin: bin,
                version: "21.0.5".to_owned(),
            })
        })
    }
}

struct Env {
    tmp: tempfile::TempDir,
    busy_ports: Arc<std::sync::Mutex<Vec<u16>>>,
}

impl Env {
    fn new() -> Self {
        Self {
            tmp: tempfile::tempdir().unwrap(),
            busy_ports: Arc::default(),
        }
    }

    fn app(&self, catalog: FakeCatalog) -> Arc<App> {
        let dirs = DataDirs::resolve(Some(self.tmp.path().to_path_buf())).unwrap();
        dirs.ensure().unwrap();
        let busy = Arc::clone(&self.busy_ports);
        let deps = AppDeps {
            catalog: Arc::new(catalog),
            java: Arc::new(FakeJava),
            port_free: Arc::new(move |p| !busy.lock().unwrap().contains(&p)),
            free_space: Arc::new(|_| Some(100 * 1024 * 1024 * 1024)),
            total_memory_mb: 16 * 1024,
            supervisor: SupervisorConfig {
                stop_timeout: Duration::from_millis(500),
                ..SupervisorConfig::default()
            },
            save_flush_timeout: Duration::from_secs(60),
        };
        Arc::new(App::with_deps(dirs, deps).unwrap())
    }

    fn server_dir(&self, spec: &InstanceSpec) -> PathBuf {
        self.tmp.path().join("servers").join(spec.id.to_string())
    }
}

fn request(name: &str) -> CreateRequest {
    CreateRequest {
        name: ServerName::parse(name).unwrap(),
        loader: LoaderKind::Paper,
        minecraft: McVersion::parse("1.21.4").unwrap(),
        memory_mb: None,
        port: None,
    }
}

async fn finish(app: &App, job: u64) -> JobState {
    let mut rx = app.jobs().subscribe(job).unwrap();
    let state = tokio::time::timeout(WAIT, rx.wait_for(|s| s.state != JobState::Running))
        .await
        .unwrap()
        .unwrap();
    state.state.clone()
}

async fn create(app: &Arc<App>, name: &str) -> InstanceSpec {
    let created = app.create(request(name)).unwrap();
    assert_eq!(finish(app, created.job_id).await, JobState::Succeeded);
    app.get(created.server_id).unwrap().spec.unwrap()
}

async fn wait_state(app: &App, spec: &InstanceSpec, state: ServerState) {
    let handle = app.handle(spec.id).unwrap();
    let status = handle.wait_for(WAIT, |s| s.state == state).await;
    assert_eq!(status.state, state, "{status:?}");
}

#[tokio::test]
async fn create_writes_all_files() {
    let env = Env::new();
    let app = env.app(FakeCatalog::default());
    let spec = create(&app, "Server của tôi").await;

    let dir = env.server_dir(&spec);
    assert_eq!(
        std::fs::read(dir.join("server/server.jar")).unwrap(),
        b"fake jar"
    );
    let props = std::fs::read_to_string(dir.join("server/server.properties")).unwrap();
    let props = mh_core::Properties::parse(&props);
    assert_eq!(props.get("motd"), Some("Server của tôi"));
    assert_eq!(props.get("server-port"), Some("25565"));
    assert!(
        !dir.join("server/eula.txt").exists(),
        "không được tự ghi EULA"
    );

    assert_eq!(spec.game.build, Some(232));
    assert_eq!(spec.game.java_major, 21);
    assert_eq!(spec.network.port, GamePort::DEFAULT);
    // 16 GB RAM: đề xuất 4096 MB.
    assert_eq!(spec.runtime.memory_mb.max_mb(), 4096);
    assert!(!spec.eula_accepted());
    let list = app.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].state, ServerState::Stopped);
}

#[tokio::test]
async fn cancelled_job_leaves_no_directory() {
    let env = Env::new();
    let gate = Arc::new(Notify::new());
    let app = env.app(FakeCatalog {
        gate: Some(Arc::clone(&gate)),
        ..FakeCatalog::default()
    });
    let created = app.create(request("A")).unwrap();
    assert_eq!(
        app.get(created.server_id).unwrap().state,
        ServerState::Installing
    );
    let dir = env
        .tmp
        .path()
        .join("servers")
        .join(created.server_id.to_string());
    // Chờ tới bước tải jar, lúc thư mục server đã được tạo.
    let mut rx = app.jobs().subscribe(created.job_id).unwrap();
    rx.wait_for(|s| {
        s.progress
            .as_ref()
            .is_some_and(|p| p.step_index == 4 && p.bytes_done.is_some())
    })
    .await
    .unwrap();
    assert!(dir.exists());

    app.jobs().cancel(created.job_id).unwrap();
    assert_eq!(finish(&app, created.job_id).await, JobState::Cancelled);
    assert!(!dir.exists());
    assert!(app.list().unwrap().is_empty());
}

#[tokio::test]
async fn failed_job_reports_code_and_cleans_up() {
    let env = Env::new();
    let app = env.app(FakeCatalog {
        fail_resolve: true,
        ..FakeCatalog::default()
    });
    let created = app.create(request("A")).unwrap();
    let state = finish(&app, created.job_id).await;
    assert!(
        matches!(
            state,
            JobState::Failed {
                code: ErrorCode::VersionNotFound,
                ..
            }
        ),
        "{state:?}"
    );
    assert!(app.list().unwrap().is_empty());
}

#[tokio::test]
async fn start_needs_eula_then_runs() {
    let env = Env::new();
    let app = env.app(FakeCatalog::default());
    let spec = create(&app, "A").await;

    let err = app.start(spec.id).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::EulaNotAccepted);

    let accepted = app.accept_eula(spec.id).unwrap();
    assert!(accepted.eula_accepted());
    let eula = std::fs::read_to_string(env.server_dir(&spec).join("server/eula.txt")).unwrap();
    assert!(eula.lines().any(|l| l == "eula=true"));

    app.start(spec.id).await.unwrap();
    wait_state(&app, &spec, ServerState::Running).await;
    app.send_command(spec.id, "say xin chào").await.unwrap();
    app.stop(spec.id).await.unwrap();
    wait_state(&app, &spec, ServerState::Stopped).await;
}

#[tokio::test]
async fn ports_are_chosen_and_conflicts_reported() {
    let env = Env::new();
    env.busy_ports.lock().unwrap().push(25566);
    let app = env.app(FakeCatalog::default());
    let a = create(&app, "A").await;
    let b = create(&app, "B").await;
    assert_eq!(a.network.port.get(), 25565);
    // 25566 bị chương trình khác chiếm.
    assert_eq!(b.network.port.get(), 25567);

    let patch = SettingsPatch {
        port: Some(GamePort::new(25565).unwrap()),
        ..SettingsPatch::default()
    };
    let err = app.update_settings(b.id, patch).unwrap_err();
    assert_eq!(err.code, ErrorCode::PortInUse);
    // Cổng của chính B không tính là bận, nên gợi ý là giữ cổng hiện tại.
    assert_eq!(err.params["suggested_port"], 25567);
}

#[tokio::test]
async fn new_port_is_written_before_next_start() {
    let env = Env::new();
    let app = env.app(FakeCatalog::default());
    let spec = create(&app, "A").await;
    app.accept_eula(spec.id).unwrap();
    let patch = SettingsPatch {
        port: Some(GamePort::new(25590).unwrap()),
        ..SettingsPatch::default()
    };
    app.update_settings(spec.id, patch).unwrap();

    app.start(spec.id).await.unwrap();
    let props = app.read_properties(spec.id).unwrap();
    assert!(props.contains(&("server-port".to_owned(), "25590".to_owned())));
    app.kill(spec.id).await.unwrap();
}

#[tokio::test]
async fn properties_can_be_replaced() {
    let env = Env::new();
    let app = env.app(FakeCatalog::default());
    let spec = create(&app, "A").await;
    let entries = vec![
        ("motd".to_owned(), "Chào".to_owned()),
        ("pvp".to_owned(), "false".to_owned()),
    ];
    app.write_properties(spec.id, &entries).unwrap();
    assert_eq!(app.read_properties(spec.id).unwrap(), entries);
    let text =
        std::fs::read_to_string(env.server_dir(&spec).join("server/server.properties")).unwrap();
    assert!(text.starts_with("# Tạo bởi MineHost"), "{text}");
}

#[tokio::test]
async fn trash_needs_matching_name_and_stopped_server() {
    let env = Env::new();
    let app = env.app(FakeCatalog::default());
    let spec = create(&app, "Thế giới").await;

    let err = app.trash(spec.id, "sai tên").unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    let dest = app.trash(spec.id, "Thế giới").unwrap();
    assert!(dest.join("instance.toml").is_file());
    assert!(app.list().unwrap().is_empty());
}

/// Bật server rồi drop app giống controller chết: process bị kill nhưng dấu `.running` còn lại.
async fn leave_running(env: &Env) -> InstanceSpec {
    let spec = {
        let app = env.app(FakeCatalog::default());
        let spec = create(&app, "A").await;
        app.accept_eula(spec.id).unwrap();
        app.start(spec.id).await.unwrap();
        wait_state(&app, &spec, ServerState::Running).await;
        spec
    };
    let marker = env.server_dir(&spec).join(".running");
    let gone = tokio::time::timeout(WAIT, async {
        while !marker.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
    gone.await.unwrap();
    spec
}

#[tokio::test]
async fn reconcile_restarts_servers_left_running() {
    let env = Env::new();
    let spec = leave_running(&env).await;
    let marker = env.server_dir(&spec).join(".running");

    let app = env.app(FakeCatalog::default());
    assert_eq!(app.reconcile().await.unwrap(), 1);
    wait_state(&app, &spec, ServerState::Running).await;
    app.shutdown_all(|_, _| {}).await;
    wait_state(&app, &spec, ServerState::Stopped).await;
    assert!(!marker.exists());
}

#[tokio::test]
async fn reconcile_waits_for_old_process_to_release_port() {
    let env = Env::new();
    let spec = leave_running(&env).await;
    // Java cũ còn giữ cổng thêm một lúc sau khi controller chết.
    let port = spec.network.port.get();
    env.busy_ports.lock().unwrap().push(port);
    let busy = Arc::clone(&env.busy_ports);
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(600));
        busy.lock().unwrap().clear();
    });

    let app = env.app(FakeCatalog::default());
    assert_eq!(app.reconcile().await.unwrap(), 1);
    release.join().unwrap();
    wait_state(&app, &spec, ServerState::Running).await;
    app.kill(spec.id).await.unwrap();
}

#[tokio::test]
async fn reconcile_does_not_wait_for_autostart_on_busy_port() {
    let env = Env::new();
    let app = env.app(FakeCatalog::default());
    let spec = create(&app, "A").await;
    app.accept_eula(spec.id).unwrap();
    let patch = SettingsPatch {
        autostart: Some(true),
        ..SettingsPatch::default()
    };
    app.update_settings(spec.id, patch).unwrap();
    drop(app);
    // Không có dấu `.running`: cổng bận là do chương trình khác, chờ cũng vô ích.
    env.busy_ports.lock().unwrap().push(spec.network.port.get());

    let app = env.app(FakeCatalog::default());
    let begun = std::time::Instant::now();
    assert_eq!(app.reconcile().await.unwrap(), 0);
    assert!(
        begun.elapsed() < Duration::from_secs(2),
        "{:?}",
        begun.elapsed()
    );
}

#[tokio::test]
async fn shutdown_all_stops_every_server() {
    let env = Env::new();
    let app = env.app(FakeCatalog::default());
    let mut specs = Vec::new();
    for name in ["A", "B", "C"] {
        let spec = create(&app, name).await;
        app.accept_eula(spec.id).unwrap();
        app.start(spec.id).await.unwrap();
        specs.push(spec);
    }
    for spec in &specs {
        wait_state(&app, spec, ServerState::Running).await;
    }
    let progress = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = Arc::clone(&progress);
    app.shutdown_all(move |done, total| seen.lock().unwrap().push((done, total)))
        .await;
    for spec in &specs {
        let status = app.handle(spec.id).unwrap().status();
        assert_eq!(status.state, ServerState::Stopped);
        // Dừng bằng `stop`, không phải bị kill: không có thông tin crash.
        assert_eq!(status.crash, None);
    }
    assert_eq!(progress.lock().unwrap().last(), Some(&(3, 3)));
}

#[tokio::test]
async fn real_app_builds_without_network() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let app = App::new(dirs).unwrap();
    assert!(app.list().unwrap().is_empty());
    assert!(app.suggested_memory().max_mb() >= 1024);
}
