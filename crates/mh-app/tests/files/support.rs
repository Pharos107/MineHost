//! Catalog và Java giả để test dịch vụ tệp không cần mạng.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mh_app::sources::{BoxFut, CatalogVersion, ProgressFn, ResolvedServer};
use mh_app::{App, AppDeps, AppError, CreateRequest, JavaSource, JobState, ServerCatalog};
use mh_core::{InstanceSpec, LoaderKind, McVersion, ServerName, ServerState};
use mh_java::JavaRuntime;
use mh_net::{CancellationToken, Checksum};
use mh_platform::DataDirs;
use mh_providers::ServerJar;
use mh_supervisor::SupervisorConfig;
use time::macros::datetime;

const WAIT: Duration = Duration::from_secs(10);

struct FakeCatalog;

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
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            progress(8, Some(8));
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

pub struct Env {
    pub tmp: tempfile::TempDir,
}

impl Env {
    pub fn new() -> Self {
        Self {
            tmp: tempfile::tempdir().unwrap(),
        }
    }

    pub fn app(&self) -> Arc<App> {
        let dirs = DataDirs::resolve(Some(self.tmp.path().to_path_buf())).unwrap();
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
            metrics: mh_app::MetricsConfig::default(),
            save_flush_timeout: Duration::from_secs(60),
        };
        Arc::new(App::with_deps(dirs, deps).unwrap())
    }

    pub fn server(&self, spec: &InstanceSpec) -> PathBuf {
        self.tmp
            .path()
            .join("servers")
            .join(spec.id.to_string())
            .join("server")
    }
}

fn request() -> CreateRequest {
    CreateRequest {
        name: ServerName::parse("A").unwrap(),
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

pub async fn create(app: &Arc<App>) -> InstanceSpec {
    let created = app.create(request()).unwrap();
    assert_eq!(finish(app, created.job_id).await, JobState::Succeeded);
    app.get(created.server_id).unwrap().spec.unwrap()
}

pub async fn wait_state(app: &App, spec: &InstanceSpec, state: ServerState) {
    let status = app
        .handle(spec.id)
        .unwrap()
        .wait_for(WAIT, |s| s.state == state)
        .await;
    assert_eq!(status.state, state, "{status:?}");
}

pub fn assert_trash(dest: &Path, files_root: &Path, rel: &str) {
    assert!(dest.starts_with(files_root), "{}", dest.display());
    let rest = dest.strip_prefix(files_root).unwrap();
    let mut parts = rest.components();
    let stamp = parts.next().unwrap().as_os_str().to_string_lossy();
    assert!(stamp.len() >= 15 && stamp.as_bytes()[8] == b'-', "{stamp}");
    assert_eq!(parts.as_path(), Path::new(rel));
}
