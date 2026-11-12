//! Giữ lại đúng số bản tự động (spec P2, mục 4.2 và 5.7).

#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mh_app::sources::{BoxFut, CatalogVersion, ProgressFn, ResolvedServer};
use mh_app::{
    App, AppDeps, AppError, BackupKind, BackupRequest, CreateRequest, JavaSource, JobState,
    MetricsConfig, ServerCatalog, SettingsPatch,
};
use mh_core::{BackupSchedule, LoaderKind, McVersion, ServerId, ServerName};
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
        name: ServerName::parse("Giữ lại").unwrap(),
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

async fn backup(app: &Arc<App>, id: ServerId, kind: BackupKind, protected: bool) {
    let req = BackupRequest {
        note: None,
        protected,
        kind,
    };
    let started = app.backup(id, req).unwrap();
    let mut rx = app.jobs().subscribe(started.job_id).unwrap();
    let state = tokio::time::timeout(WAIT, rx.wait_for(|s| s.state != JobState::Running))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.state, JobState::Succeeded, "{state:?}");
    // `created_at` tính theo giây: tách các bản ra để thứ tự mới nhất trước là chắc chắn.
    tokio::time::sleep(Duration::from_millis(1100)).await;
}

#[tokio::test]
async fn scheduled_backups_keep_newest_n_and_spare_manual_and_protected() {
    let (_tmp, app) = test_app();
    let id = create(&app).await;
    let patch = SettingsPatch {
        backup_schedule: Some(BackupSchedule::parse("every_6h").unwrap()),
        backup_keep: Some(2),
        ..SettingsPatch::default()
    };
    app.update_settings(id, patch).unwrap();

    backup(&app, id, BackupKind::Manual, false).await;
    backup(&app, id, BackupKind::Scheduled, true).await;
    for _ in 0..3 {
        backup(&app, id, BackupKind::Scheduled, false).await;
    }

    let list = app.list_backups(id).unwrap();
    let scheduled = list
        .iter()
        .filter(|m| m.kind == BackupKind::Scheduled && !m.protected)
        .count();
    assert_eq!(scheduled, 2, "{list:#?}");
    assert!(list.iter().any(|m| m.kind == BackupKind::Manual));
    assert!(
        list.iter()
            .any(|m| m.kind == BackupKind::Scheduled && m.protected)
    );

    let candidates = app.backup_candidates();
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].last,
        Some(
            list.iter()
                .filter(|m| m.kind == BackupKind::Scheduled)
                .map(|m| m.created_at.unix_timestamp())
                .max()
                .unwrap()
        )
    );
}

#[tokio::test]
async fn bad_keep_is_rejected() {
    let (_tmp, app) = test_app();
    let id = create(&app).await;
    for keep in [0, 101] {
        let patch = SettingsPatch {
            backup_keep: Some(keep),
            ..SettingsPatch::default()
        };
        assert!(app.update_settings(id, patch).is_err(), "{keep}");
    }
}
