//! Bộ nhớ đệm tải về (spec P3, mục 3.4, 6.5): đo dung lượng, xóa, chặn xóa khi đang có Job tải.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mh_app::sources::{BoxFut, CatalogVersion, ProgressFn, ResolvedServer};
use mh_app::{
    App, AppDeps, AppError, CreateRequest, JavaSource, JobState, MetricsConfig, ServerCatalog,
};
use mh_core::{ErrorCode, LoaderKind, McVersion, ServerName};
use mh_java::JavaRuntime;
use mh_net::{CancellationToken, Checksum};
use mh_platform::DataDirs;
use mh_providers::ServerJar;
use mh_supervisor::SupervisorConfig;
use tokio::sync::Notify;

/// Tải jar thì chờ `release`, để Job tạo server còn chạy trong lúc test thử xóa cache.
struct HeldCatalog {
    release: Arc<Notify>,
}

impl ServerCatalog for HeldCatalog {
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
            self.release.notified().await;
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
            Ok(JavaRuntime {
                major,
                home: PathBuf::from("none"),
                java_bin: PathBuf::from("none/java"),
                version: "21.0.5".to_owned(),
            })
        })
    }
}

fn app(release: Arc<Notify>) -> (tempfile::TempDir, Arc<App>) {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let deps = AppDeps {
        catalog: Arc::new(HeldCatalog { release }),
        java: Arc::new(FakeJava),
        port_free: Arc::new(|_| true),
        free_space: Arc::new(|_| Some(100 * 1024 * 1024 * 1024)),
        total_memory_mb: 16 * 1024,
        supervisor: SupervisorConfig::default(),
        save_flush_timeout: Duration::from_secs(60),
        metrics: MetricsConfig::default(),
    };
    (tmp, Arc::new(App::with_deps(dirs, deps).unwrap()))
}

fn put(root: &Path, rel: &str, bytes: usize) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![7u8; bytes]).unwrap();
}

#[tokio::test]
async fn size_counts_hash_folders_and_clear_keeps_meta() {
    let (tmp, app) = app(Arc::new(Notify::new()));
    let cache = tmp.path().join("cache");
    put(&cache, "sha1/ab/abcdef", 100);
    put(&cache, "sha512/cd/cdef01", 50);
    put(&cache, "meta/modrinth.json", 1000);
    assert_eq!(app.download_cache_size(), 150);

    assert_eq!(app.clear_download_cache().unwrap(), 150);
    assert_eq!(app.download_cache_size(), 0);
    assert!(!cache.join("sha1").exists());
    assert!(cache.join("meta/modrinth.json").is_file());
    // Xóa lần nữa khi đã trống vẫn ổn.
    assert_eq!(app.clear_download_cache().unwrap(), 0);
}

#[tokio::test]
async fn clearing_while_a_job_downloads_is_server_busy() {
    let release = Arc::new(Notify::new());
    let (tmp, app) = app(Arc::clone(&release));
    put(&tmp.path().join("cache"), "sha256/00/0011", 10);
    let created = app
        .create(CreateRequest {
            name: ServerName::parse("Đang tải").unwrap(),
            loader: LoaderKind::Paper,
            minecraft: McVersion::parse("1.21.4").unwrap(),
            loader_version: None,
            memory_mb: None,
            port: None,
        })
        .unwrap();

    let err = app.clear_download_cache().unwrap_err();
    assert_eq!(err.code, ErrorCode::ServerBusy);
    assert_eq!(app.download_cache_size(), 10);

    release.notify_one();
    let mut rx = app.jobs().subscribe(created.job_id).unwrap();
    let state = tokio::time::timeout(
        Duration::from_secs(10),
        rx.wait_for(|s| s.state != JobState::Running),
    )
    .await
    .unwrap()
    .unwrap()
    .state
    .clone();
    assert_eq!(state, JobState::Succeeded);
    assert_eq!(app.clear_download_cache().unwrap(), 10);
}
