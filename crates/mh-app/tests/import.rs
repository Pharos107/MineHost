//! Nhập server có sẵn (spec P2, mục 6.11) với catalog và Java giả.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mh_app::sources::{BoxFut, CatalogVersion, ProgressFn, ResolvedServer};
use mh_app::{
    App, AppDeps, AppError, ImportRequest, JavaSource, JobState, MetricsConfig, ServerCatalog,
};
use mh_core::{ErrorCode, LoaderKind, McVersion, ServerId, ServerName};
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

fn request(path: &Path) -> ImportRequest {
    ImportRequest {
        path: path.to_path_buf(),
        name: ServerName::parse("Nhập").unwrap(),
        loader: None,
        minecraft: None,
    }
}

/// Thư mục giống server Paper đã chạy vài lần: có `version_history.json`, jar, world, cổng riêng.
fn paper_folder() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("version_history.json"),
        r#"{"currentVersion":"git-Paper-232 (MC: 1.21.4)"}"#,
    )
    .unwrap();
    std::fs::write(root.join("paper-1.21.4-232.jar"), b"jar").unwrap();
    std::fs::write(
        root.join("server.properties"),
        "motd=Cũ
server-port=25570
",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("world/region")).unwrap();
    std::fs::write(root.join("world/level.dat"), b"level").unwrap();
    std::fs::write(root.join("world/region/r.0.0.mca"), vec![7u8; 4096]).unwrap();
    dir
}

async fn finish(app: &Arc<App>, req: ImportRequest) -> ServerId {
    let created = app.import(req).unwrap();
    let mut rx = app.jobs().subscribe(created.job_id).unwrap();
    let state = tokio::time::timeout(WAIT, rx.wait_for(|s| s.state != JobState::Running))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.state, JobState::Succeeded, "{state:?}");
    created.server_id
}

#[tokio::test]
async fn paper_folder_is_copied_with_detected_version() {
    let (_tmp, app) = test_app();
    let source = paper_folder();
    let id = finish(&app, request(source.path())).await;

    let view = app.get(id).unwrap();
    let spec = view.spec.unwrap();
    assert_eq!(spec.game.loader, LoaderKind::Paper);
    assert_eq!(spec.game.minecraft, McVersion::parse("1.21.4").unwrap());
    assert_eq!(spec.game.jar.as_str(), "paper-1.21.4-232.jar");
    assert_eq!(spec.network.port.get(), 25570);
    assert!(
        spec.eula.accepted_at.is_none(),
        "EULA phải được người dùng xác nhận lại"
    );

    let props = app.read_properties(id).unwrap();
    assert!(props.contains(&("motd".to_owned(), "Cũ".to_owned())));
    // Bản gốc giữ nguyên.
    assert!(source.path().join("world/level.dat").is_file());
    let files = app.list_dir(id, "world/region").await.unwrap();
    assert_eq!(files.len(), 1);
}

#[tokio::test]
async fn mods_folder_is_refused() {
    let (_tmp, app) = test_app();
    let source = paper_folder();
    std::fs::create_dir(source.path().join("mods")).unwrap();
    let err = app.import(request(source.path())).unwrap_err();
    assert_eq!(err.code, ErrorCode::ModsNotSupported);
}

#[tokio::test]
async fn unknown_folder_needs_manual_choice() {
    let (_tmp, app) = test_app();
    let source = tempfile::tempdir().unwrap();
    std::fs::write(source.path().join("custom.jar"), b"not a zip").unwrap();

    let found = app.inspect_import(source.path()).unwrap();
    assert_eq!((found.loader, found.minecraft), (None, None));
    let err = app.import(request(source.path())).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert_eq!(err.params["field"], "loader");

    let mut req = request(source.path());
    req.loader = Some(LoaderKind::Vanilla);
    req.minecraft = Some(McVersion::parse("1.20.4").unwrap());
    let id = finish(&app, req).await;
    assert_eq!(
        app.get(id).unwrap().spec.unwrap().game.jar.as_str(),
        "custom.jar"
    );
}

#[tokio::test]
async fn folder_inside_data_dir_or_relative_is_refused() {
    let (tmp, app) = test_app();
    let inside = tmp.path().join("servers");
    let err = app.inspect_import(&inside).unwrap_err();
    assert_eq!(err.params["field"], "path");
    let err = app.inspect_import(Path::new("relative/dir")).unwrap_err();
    assert_eq!(err.params["field"], "path");
}
