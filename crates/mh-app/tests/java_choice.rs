//! Chọn phiên bản Java cho từng server và tự tải qua Job khi bật server nếu thiếu.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use mh_app::jobs::{JobKind, JobStep};
use mh_app::sources::{BoxFut, CatalogVersion, ProgressFn, ResolvedServer};
use mh_app::{
    App, AppDeps, AppError, CreateRequest, JavaSource, JobState, MetricsConfig, ServerCatalog,
    SettingsPatch,
};
use mh_core::{ErrorCode, LoaderKind, McVersion, MemoryLimits, ServerName};
use mh_java::JavaRuntime;
use mh_net::{CancellationToken, Checksum};
use mh_platform::DataDirs;
use mh_providers::ServerJar;
use mh_supervisor::SupervisorConfig;

struct PartialJava {
    has_17: AtomicBool,
}

impl JavaSource for PartialJava {
    fn is_installed(&self, major: u16) -> bool {
        major != 17 || self.has_17.load(Ordering::Relaxed)
    }

    fn ensure<'a>(
        &'a self,
        major: u16,
        progress: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<JavaRuntime, AppError>> {
        Box::pin(async move {
            progress(50, Some(100));
            if major == 17 {
                self.has_17.store(true, Ordering::Relaxed);
            }
            let bin = PathBuf::from(env!("CARGO_BIN_EXE_app-fake-mc"));
            Ok(JavaRuntime {
                major,
                home: bin.parent().unwrap().to_path_buf(),
                java_bin: bin,
                version: format!("{major}.0.1"),
            })
        })
    }
}

struct FakeCatalog;

impl ServerCatalog for FakeCatalog {
    fn versions<'a>(&'a self, _: LoaderKind) -> BoxFut<'a, Result<Vec<CatalogVersion>, AppError>> {
        Box::pin(async { Ok(vec![]) })
    }

    fn resolve<'a>(
        &'a self,
        _: LoaderKind,
        mc: &'a McVersion,
    ) -> BoxFut<'a, Result<ResolvedServer, AppError>> {
        Box::pin(async move {
            Ok(ResolvedServer {
                minecraft: mc.clone(),
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

#[tokio::test]
async fn java_major_validation_and_missing_java_job_on_start() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let deps = AppDeps {
        catalog: Arc::new(FakeCatalog),
        java: Arc::new(PartialJava {
            has_17: AtomicBool::new(false),
        }),
        port_free: Arc::new(|_| true),
        free_space: Arc::new(|_| Some(100 * 1024 * 1024 * 1024)),
        total_memory_mb: 16 * 1024,
        supervisor: SupervisorConfig {
            stop_timeout: Duration::from_millis(500),
            ..SupervisorConfig::default()
        },
        save_flush_timeout: Duration::from_secs(60),
        metrics: MetricsConfig::default(),
    };
    let app = Arc::new(App::with_deps(dirs, deps).unwrap());

    let created = app
        .create(CreateRequest {
            name: ServerName::parse("JavaTest").unwrap(),
            loader: LoaderKind::Paper,
            minecraft: McVersion::parse("1.21.4").unwrap(),
            loader_version: None,
            memory_mb: Some(MemoryLimits::new(1024, 1024).unwrap()),
            port: None,
        })
        .unwrap();
    let server_id = created.server_id;
    let mut watch = app.jobs().subscribe(created.job_id).unwrap();
    watch
        .wait_for(|s| s.state != JobState::Running)
        .await
        .unwrap();

    // Không hỗ trợ Java 19.
    let bad = app
        .update_settings(
            server_id,
            SettingsPatch {
                java_major: Some(19),
                ..SettingsPatch::default()
            },
        )
        .unwrap_err();
    assert_eq!(bad.code, ErrorCode::InvalidInput);
    assert_eq!(bad.params["field"], "java_major");

    // Đổi sang Java 17 (chưa cài) -> bật server chạy Job InstallJava.
    app.update_settings(
        server_id,
        SettingsPatch {
            java_major: Some(17),
            ..SettingsPatch::default()
        },
    )
    .unwrap();
    app.accept_eula(server_id).unwrap();
    app.start(server_id).await.unwrap();

    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let jobs = app.jobs().list(Some(server_id));
            if let Some(job) = jobs.iter().find(|j| j.kind == JobKind::InstallJava) {
                assert_eq!(
                    job.progress.as_ref().map(|p| &p.step),
                    Some(&JobStep::DownloadJava { major: 17 })
                );
                if job.state == JobState::Succeeded {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();

    app.kill(server_id).await.unwrap();
}
