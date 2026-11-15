//! Bản `before_update` và bản chụp `content.toml`, `instance.toml` trong backup (spec P3, mục 3.9, 4.2).

#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mh_app::sources::{BoxFut, CatalogVersion, ProgressFn, ResolvedServer};
use mh_app::{
    App, AppDeps, AppError, BackupKind, BackupRequest, CreateRequest, JavaSource, JobState,
    ServerCatalog, SettingsPatch,
};
use mh_core::{BackupSchedule, LoaderKind, McVersion, ServerId, ServerName};
use mh_java::JavaRuntime;
use mh_net::{CancellationToken, Checksum};
use mh_platform::DataDirs;
use mh_providers::ServerJar;
use mh_supervisor::SupervisorConfig;

const WAIT: Duration = Duration::from_secs(20);

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
        metrics: mh_app::MetricsConfig::default(),
    };
    (tmp, Arc::new(App::with_deps(dirs, deps).unwrap()))
}

async fn finish(app: &App, job: u64) {
    let mut rx = app.jobs().subscribe(job).unwrap();
    let state = tokio::time::timeout(WAIT, rx.wait_for(|s| s.state != JobState::Running))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.state, JobState::Succeeded, "{state:?}");
}

async fn create(app: &Arc<App>) -> ServerId {
    let created = app
        .create(CreateRequest {
            name: ServerName::parse("Cập nhật").unwrap(),
            loader: LoaderKind::Paper,
            minecraft: McVersion::parse("1.21.4").unwrap(),
            loader_version: None,
            memory_mb: None,
            port: None,
        })
        .unwrap();
    finish(app, created.job_id).await;
    created.server_id
}

async fn backup(app: &Arc<App>, id: ServerId, kind: BackupKind) -> String {
    let req = BackupRequest {
        note: None,
        protected: false,
        kind,
    };
    let started = app.backup(id, req).unwrap();
    finish(app, started.job_id).await;
    // `created_at` tính theo giây: tách các bản ra để thứ tự mới nhất trước là chắc chắn.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    app.list_backups(id).unwrap()[0].id.clone()
}

fn content_file(tmp: &Path, id: ServerId) -> PathBuf {
    tmp.join("servers")
        .join(id.to_string())
        .join("content.toml")
}

#[tokio::test]
async fn before_update_is_protected_and_outlives_keep() {
    let (_tmp, app) = test_app();
    let id = create(&app).await;
    let patch = SettingsPatch {
        backup_schedule: Some(BackupSchedule::parse("every_6h").unwrap()),
        backup_keep: Some(1),
        ..SettingsPatch::default()
    };
    app.update_settings(id, patch).unwrap();

    let before = backup(&app, id, BackupKind::BeforeUpdate).await;
    for _ in 0..2 {
        backup(&app, id, BackupKind::Scheduled).await;
    }

    let list = app.list_backups(id).unwrap();
    let kept = list.iter().find(|m| m.id == before).unwrap();
    assert_eq!(kept.kind, BackupKind::BeforeUpdate);
    assert!(kept.protected, "{kept:?}");
    let scheduled = list
        .iter()
        .filter(|m| m.kind == BackupKind::Scheduled)
        .count();
    assert_eq!(scheduled, 1, "{list:#?}");
    let err = app.delete_backup(id, &before).unwrap_err();
    assert_eq!(err.code, mh_core::ErrorCode::BackupProtected);
}

#[tokio::test]
async fn restore_brings_back_content_toml_and_game_but_keeps_name() {
    let (tmp, app) = test_app();
    let id = create(&app).await;
    let content = content_file(tmp.path(), id);
    std::fs::write(&content, "schema = 1\n# trước khi cập nhật\n").unwrap();
    let chosen = backup(&app, id, BackupKind::BeforeUpdate).await;

    // Giả lập một lần cập nhật: `content.toml` và `[game]` đổi, người dùng đổi tên sau đó.
    std::fs::write(&content, "schema = 1\n# sau khi cập nhật\n").unwrap();
    let patch = SettingsPatch {
        name: Some(ServerName::parse("Tên mới").unwrap()),
        java_major: Some(17),
        ..SettingsPatch::default()
    };
    app.update_settings(id, patch).unwrap();

    let started = app.restore(id, &chosen).unwrap();
    finish(&app, started.job_id).await;

    assert_eq!(
        std::fs::read_to_string(&content).unwrap(),
        "schema = 1\n# trước khi cập nhật\n"
    );
    let spec = app.get(id).unwrap().spec.unwrap();
    assert_eq!(spec.game.java_major, 21);
    assert_eq!(spec.name.as_str(), "Tên mới");
    let server_dir = tmp
        .path()
        .join("servers")
        .join(id.to_string())
        .join("server");
    assert!(!server_dir.join(".minehost").exists());
}

#[tokio::test]
async fn restore_removes_content_toml_that_did_not_exist_at_backup_time() {
    let (tmp, app) = test_app();
    let id = create(&app).await;
    let content = content_file(tmp.path(), id);
    let _ = std::fs::remove_file(&content);
    let chosen = backup(&app, id, BackupKind::Manual).await;
    std::fs::write(&content, "schema = 1\n").unwrap();

    let started = app.restore(id, &chosen).unwrap();
    finish(&app, started.job_id).await;
    assert!(!content.exists());
}

#[tokio::test]
async fn world_folder_named_like_the_snapshot_is_not_archived() {
    let (tmp, app) = test_app();
    let id = create(&app).await;
    let server_dir = tmp
        .path()
        .join("servers")
        .join(id.to_string())
        .join("server");
    std::fs::create_dir_all(server_dir.join(".minehost")).unwrap();
    std::fs::write(server_dir.join(".minehost/content.toml"), "giả").unwrap();
    let content = content_file(tmp.path(), id);
    std::fs::write(&content, "schema = 1\n").unwrap();
    let chosen = backup(&app, id, BackupKind::Manual).await;

    let started = app.restore(id, &chosen).unwrap();
    finish(&app, started.job_id).await;
    assert_eq!(std::fs::read_to_string(&content).unwrap(), "schema = 1\n");
}
