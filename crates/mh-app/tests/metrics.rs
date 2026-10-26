//! Lấy mẫu chỉ số với `fake-mc`. Chu kỳ rút ngắn để không chờ 5 giây.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mh_app::sources::{BoxFut, CatalogVersion, ProgressFn, ResolvedServer};
use mh_app::{
    App, AppDeps, AppError, CreateRequest, JavaSource, JobState, MetricsConfig, ServerCatalog,
    ServerMetrics,
};
use mh_core::{ErrorCode, LoaderKind, McVersion, ServerId, ServerName, ServerState};
use mh_java::JavaRuntime;
use mh_net::{CancellationToken, Checksum};
use mh_platform::DataDirs;
use mh_providers::ServerJar;
use mh_supervisor::SupervisorConfig;

const WAIT: Duration = Duration::from_secs(10);
const TICK: Duration = Duration::from_millis(50);

struct FakeCatalog;

impl ServerCatalog for FakeCatalog {
    fn versions(&self, _: LoaderKind) -> BoxFut<'_, Result<Vec<CatalogVersion>, AppError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn resolve<'a>(
        &'a self,
        _: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<ResolvedServer, AppError>> {
        Box::pin(async move {
            Ok(ResolvedServer {
                minecraft: minecraft.clone(),
                build: Some(1),
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
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            std::fs::write(dest, b"fake jar").unwrap();
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

fn test_app() -> (tempfile::TempDir, Arc<App>) {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let deps = AppDeps {
        catalog: Arc::new(FakeCatalog),
        java: Arc::new(FakeJava),
        port_free: Arc::new(|_| true),
        free_space: Arc::new(|_| Some(100 * 1024 * 1024 * 1024)),
        total_memory_mb: 16 * 1024,
        supervisor: SupervisorConfig {
            stop_timeout: Duration::from_millis(500),
            ..SupervisorConfig::default()
        },
        metrics: MetricsConfig {
            sample_every: TICK,
            disk_every: TICK,
        },
        save_flush_timeout: Duration::from_secs(60),
    };
    let app = Arc::new(App::with_deps(dirs, deps).unwrap());
    app.spawn_metrics();
    (tmp, app)
}

async fn create(app: &Arc<App>) -> ServerId {
    let req = CreateRequest {
        name: ServerName::parse("Chỉ số").unwrap(),
        loader: LoaderKind::Paper,
        minecraft: McVersion::parse("1.21.4").unwrap(),
        memory_mb: None,
        port: None,
    };
    let created = app.create(req).unwrap();
    let mut rx = app.jobs().subscribe(created.job_id).unwrap();
    let state = tokio::time::timeout(WAIT, rx.wait_for(|s| s.state != JobState::Running))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.state, JobState::Succeeded);
    app.accept_eula(created.server_id).unwrap();
    created.server_id
}

async fn wait_state(app: &App, id: ServerId, state: ServerState) {
    let status = app
        .handle(id)
        .unwrap()
        .wait_for(WAIT, |s| s.state == state)
        .await;
    assert_eq!(status.state, state, "{status:?}");
}

async fn wait_samples(app: &App, id: ServerId) -> ServerMetrics {
    let deadline = std::time::Instant::now() + WAIT;
    loop {
        let metrics = app.metrics(id, None).unwrap();
        let ready = metrics.samples.len() >= 2
            && metrics.running_since.is_some()
            && metrics.disk_bytes.is_some_and(|n| n > 0)
            && metrics.samples.iter().any(|sample| sample.memory_bytes > 0);
        if ready {
            return metrics;
        }
        if std::time::Instant::now() > deadline {
            panic!("không có đủ mẫu: {metrics:?}");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Ba lần đọc liên tiếp không đổi: process đã tắt nên task không thêm mẫu.
async fn assert_no_new_samples(app: &App, id: ServerId) {
    let mut previous = app.metrics(id, None).unwrap();
    let mut stable = 0;
    for _ in 0..40 {
        tokio::time::sleep(TICK).await;
        let now = app.metrics(id, None).unwrap();
        let same = now.samples.len() == previous.samples.len()
            && now.samples.last().map(|sample| sample.at) == previous.samples.last().map(|s| s.at);
        if same {
            stable += 1;
            if stable >= 3 {
                assert!(!now.samples.is_empty());
                return;
            }
        } else {
            stable = 0;
            previous = now;
        }
    }
    panic!("vẫn thêm mẫu sau khi tắt ({})", previous.samples.len());
}

#[tokio::test]
async fn running_server_is_sampled_until_it_stops() {
    let (_tmp, app) = test_app();
    let id = create(&app).await;
    let missing = ServerId::generate();
    assert_eq!(
        app.metrics(missing, None).unwrap_err().code,
        ErrorCode::NotFound
    );

    app.start(id).await.unwrap();
    wait_state(&app, id, ServerState::Running).await;
    let full = wait_samples(&app, id).await;
    assert!(
        full.samples.iter().all(|sample| {
            sample.players.is_none() && (0.0..=100.0).contains(&sample.cpu_percent)
        })
    );

    let first = full.samples[0].at;
    let filtered = app.metrics(id, Some(first)).unwrap();
    assert!(filtered.samples.iter().all(|sample| sample.at > first));
    for sample in full.samples.iter().filter(|sample| sample.at > first) {
        assert!(filtered.samples.iter().any(|got| got.at == sample.at));
    }
    let early = first - time::Duration::seconds(60);
    let all = app.metrics(id, Some(early)).unwrap();
    assert!(all.samples.len() >= full.samples.len());

    app.stop(id).await.unwrap();
    wait_state(&app, id, ServerState::Stopped).await;
    let stopped = app.metrics(id, None).unwrap();
    assert!(stopped.running_since.is_none());
    assert!(!stopped.samples.is_empty(), "mẫu cũ còn sau khi tắt");
    assert_no_new_samples(&app, id).await;
}
