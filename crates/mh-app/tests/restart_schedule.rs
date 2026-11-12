//! Lịch khởi động lại với `fake-mc` và đồng hồ giả (spec P2, kế hoạch PR-31).

#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mh_app::sources::{BoxFut, CatalogVersion, ProgressFn, ResolvedServer};
use mh_app::{
    App, AppDeps, AppError, CreateRequest, JavaSource, JobState, MetricsConfig, RestartAction,
    RestartPlanner, ServerCatalog, SettingsPatch,
};
use mh_core::{DailyTime, LoaderKind, McVersion, ServerId, ServerName, ServerState};
use mh_java::JavaRuntime;
use mh_net::{CancellationToken, Checksum};
use mh_platform::DataDirs;
use mh_providers::ServerJar;
use mh_supervisor::SupervisorConfig;

const WAIT: Duration = Duration::from_secs(10);
const TICK: Duration = Duration::from_millis(50);
/// 04:00 giờ máy, và một mốc giây Unix giả đi cùng.
const FOUR: u32 = 4 * 3600;
const BASE: i64 = 1_000_000;

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
        save_flush_timeout: Duration::from_secs(60),
        metrics: MetricsConfig {
            sample_every: TICK,
            disk_every: TICK,
        },
    };
    let app = Arc::new(App::with_deps(dirs, deps).unwrap());
    (tmp, app)
}

async fn create(app: &Arc<App>) -> ServerId {
    let req = CreateRequest {
        name: ServerName::parse("Lịch").unwrap(),
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

async fn with_schedule(app: &Arc<App>, id: ServerId) {
    let patch = SettingsPatch {
        restart_at: Some(vec![DailyTime::parse("04:00").unwrap()]),
        ..SettingsPatch::default()
    };
    app.update_settings(id, patch).unwrap();
}

/// Chạy lịch từng giây trong `[from, to]` với đồng hồ giả, trả về các việc đã làm.
async fn drive(app: &App, planner: &mut RestartPlanner, from: u32, to: u32) -> Vec<RestartAction> {
    let mut done = Vec::new();
    for second in from..=to {
        let schedules = app.restart_schedules();
        for action in planner.tick(BASE + i64::from(second), second, &schedules) {
            app.apply_restart_action(&action).await;
            done.push(action);
        }
    }
    done
}

fn says(app: &App, id: ServerId) -> usize {
    let tail = app.handle(id).unwrap().console().tail(500);
    tail.iter()
        .filter(|line| line.contains("[Server] [MineHost] Server"))
        .count()
}

#[tokio::test]
async fn three_says_then_restart() {
    let (_tmp, app) = test_app();
    let id = create(&app).await;
    with_schedule(&app, id).await;
    app.start(id).await.unwrap();
    wait_state(&app, id, ServerState::Running).await;
    let first_pid = app.handle(id).unwrap().status().pid;

    let mut planner = RestartPlanner::default();
    let warned = drive(&app, &mut planner, FOUR - 400, FOUR - 1).await;
    assert_eq!(warned.len(), 3, "{warned:?}");
    let deadline = std::time::Instant::now() + WAIT;
    while says(&app, id) < 3 {
        assert!(
            std::time::Instant::now() < deadline,
            "fake-mc chưa in đủ 3 lời báo"
        );
        tokio::time::sleep(TICK).await;
    }

    let restarted = drive(&app, &mut planner, FOUR, FOUR + 5).await;
    assert_eq!(restarted, [RestartAction::Restart(id)]);
    let status = app
        .handle(id)
        .unwrap()
        .wait_for(WAIT, |s| {
            s.state == ServerState::Running && s.pid != first_pid
        })
        .await;
    assert_eq!(status.state, ServerState::Running, "{status:?}");
    assert_ne!(status.pid, first_pid);
    app.kill(id).await.unwrap();
}

#[tokio::test]
async fn stopped_server_is_left_alone() {
    let (_tmp, app) = test_app();
    let id = create(&app).await;
    with_schedule(&app, id).await;

    let mut planner = RestartPlanner::default();
    assert!(
        drive(&app, &mut planner, FOUR - 400, FOUR + 5)
            .await
            .is_empty()
    );
    assert_eq!(app.handle(id).unwrap().status().state, ServerState::Stopped);
}
