//! Job khôi phục với `fake-mc`: tắt server đang chạy, bản `before_restore`, và world không đổi khi hỏng.

#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mh_app::jobs::JobKind;
use mh_app::sources::{BoxFut, CatalogVersion, ProgressFn, ResolvedServer};
use mh_app::{
    App, AppDeps, AppError, BackupKind, BackupRequest, CreateRequest, JavaSource, JobState,
    ServerCatalog,
};
use mh_core::{
    ConsoleStream, ErrorCode, InstanceSpec, LoaderKind, McVersion, ServerId, ServerName,
    ServerState,
};
use mh_java::JavaRuntime;
use mh_net::{CancellationToken, Checksum};
use mh_platform::DataDirs;
use mh_providers::ServerJar;
use mh_supervisor::SupervisorConfig;
use serde_json::Value;
use time::macros::datetime;

const WAIT: Duration = Duration::from_secs(20);

#[derive(Default)]
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

struct Env {
    tmp: tempfile::TempDir,
}

impl Env {
    fn new() -> Self {
        Self {
            tmp: tempfile::tempdir().unwrap(),
        }
    }

    fn app(&self) -> Arc<App> {
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
            save_flush_timeout: Duration::from_secs(60),
            metrics: mh_app::MetricsConfig::default(),
        };
        Arc::new(App::with_deps(dirs, deps).unwrap())
    }

    fn server_dir(&self, spec: &InstanceSpec) -> PathBuf {
        self.tmp
            .path()
            .join("servers")
            .join(spec.id.to_string())
            .join("server")
    }

    fn backup_dir(&self, id: ServerId) -> PathBuf {
        self.tmp.path().join("backups").join(id.to_string())
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

fn manual() -> BackupRequest {
    BackupRequest {
        note: None,
        protected: false,
        kind: BackupKind::Manual,
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

async fn boot(app: &App, spec: &InstanceSpec) {
    app.accept_eula(spec.id).unwrap();
    app.start(spec.id).await.unwrap();
    let status = app
        .handle(spec.id)
        .unwrap()
        .wait_for(WAIT, |s| s.state == ServerState::Running)
        .await;
    assert_eq!(status.state, ServerState::Running, "{status:?}");
}

fn stdin_commands(app: &App, id: ServerId) -> Vec<String> {
    app.handle(id)
        .unwrap()
        .console()
        .after(None)
        .into_iter()
        .filter(|line| line.stream == ConsoleStream::Stdin)
        .map(|line| line.text)
        .collect()
}

fn corrupt_sha(path: &Path) {
    let mut value: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    value["sha256"] = Value::String("0".repeat(64));
    std::fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

fn staged_in(root: &Path) -> Vec<String> {
    std::fs::read_dir(root)
        .unwrap()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(".restore-") || name.starts_with(".old-"))
        .collect()
}

fn write_tree(dir: &Path, file: &str, body: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(file), body).unwrap();
}

#[tokio::test]
async fn running_server_is_stopped_and_world_matches_backup() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app, "Restore").await;
    boot(&app, &spec).await;
    let dir = env.server_dir(&spec);
    std::fs::write(dir.join("kept.txt"), b"from-backup").unwrap();
    let backed = app.backup(spec.id, manual()).unwrap();
    assert_eq!(finish(&app, backed.job_id).await, JobState::Succeeded);
    let chosen = app.list_backups(spec.id).unwrap().remove(0);
    assert_eq!(chosen.kind, BackupKind::Manual);
    std::fs::write(dir.join("kept.txt"), b"mutated").unwrap();
    std::fs::write(dir.join("extra.txt"), b"new").unwrap();
    let commands = stdin_commands(&app, spec.id);

    let started = app.restore(spec.id, &chosen.id).unwrap();
    assert_eq!(finish(&app, started.job_id).await, JobState::Succeeded);

    assert_eq!(std::fs::read(dir.join("kept.txt")).unwrap(), b"from-backup");
    assert!(!dir.join("extra.txt").exists());
    let status = app.handle(spec.id).unwrap().status();
    assert_eq!(status.state, ServerState::Stopped, "{status:?}");
    let safety = app
        .list_backups(spec.id)
        .unwrap()
        .into_iter()
        .find(|meta| meta.kind == BackupKind::BeforeRestore)
        .unwrap();
    assert_ne!(safety.sha256, chosen.sha256);
    let added = &stdin_commands(&app, spec.id)[commands.len()..];
    assert!(
        added.iter().all(|line| !line.starts_with("save-")),
        "{added:?}"
    );
    let root = dir.parent().unwrap();
    assert!(staged_in(root).is_empty(), "{:?}", staged_in(root));
}

#[tokio::test]
async fn corrupt_descriptor_leaves_server_unchanged() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app, "Corrupt").await;
    let dir = env.server_dir(&spec);
    std::fs::write(dir.join("kept.txt"), b"stay").unwrap();
    let backed = app.backup(spec.id, manual()).unwrap();
    assert_eq!(finish(&app, backed.job_id).await, JobState::Succeeded);
    let chosen = app.list_backups(spec.id).unwrap().remove(0);
    corrupt_sha(&env.backup_dir(spec.id).join(format!("{}.json", chosen.id)));

    let started = app.restore(spec.id, &chosen.id).unwrap();
    let state = finish(&app, started.job_id).await;
    assert!(
        matches!(
            state,
            JobState::Failed {
                code: ErrorCode::BackupCorrupt,
                ..
            }
        ),
        "{state:?}"
    );
    assert_eq!(std::fs::read(dir.join("kept.txt")).unwrap(), b"stay");
    let root = dir.parent().unwrap();
    assert!(staged_in(root).is_empty(), "{:?}", staged_in(root));
}

#[tokio::test]
async fn missing_backup_is_not_found_without_a_job() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app, "Missing").await;
    let missing = ulid::Ulid::generate().to_string();
    let err = app.restore(spec.id, &missing).unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
    assert!(
        app.jobs()
            .list(Some(spec.id))
            .iter()
            .all(|job| job.kind != JobKind::Restore)
    );
}

#[tokio::test]
async fn startup_and_reconcile_remove_interrupted_restore_dirs() {
    let env = Env::new();
    let servers = env.tmp.path().join("servers");
    plant(&servers);
    let app = env.app();
    assert_clean(&servers);

    let later = servers.join("later");
    write_tree(&later.join("server"), "live.txt", "stay");
    write_tree(&later.join(".restore-02"), "junk.txt", "x");
    write_tree(&later.join(".old-02"), "gone.txt", "old");
    app.reconcile().await.unwrap();
    assert_eq!(
        std::fs::read(later.join("server").join("live.txt")).unwrap(),
        b"stay"
    );
    assert!(staged_in(&later).is_empty(), "{:?}", staged_in(&later));
}

fn plant(servers: &Path) {
    let intact = servers.join("intact");
    write_tree(&intact.join("server"), "live.txt", "stay");
    write_tree(&intact.join(".restore-01"), "junk.txt", "x");
    write_tree(&intact.join(".old-01"), "gone.txt", "old");

    let rolled = servers.join("rolled");
    write_tree(
        &rolled.join(".old-00000000000000000000000000"),
        "world.txt",
        "older",
    );
    write_tree(
        &rolled.join(".old-7ZZZZZZZZZZZZZZZZZZZZZZZZZ"),
        "world.txt",
        "newest",
    );
    write_tree(&rolled.join(".restore-01"), "partial.txt", "nope");
}

fn assert_clean(servers: &Path) {
    let intact = servers.join("intact");
    assert_eq!(
        std::fs::read(intact.join("server").join("live.txt")).unwrap(),
        b"stay"
    );
    assert!(staged_in(&intact).is_empty(), "{:?}", staged_in(&intact));
    let rolled = servers.join("rolled");
    assert_eq!(
        std::fs::read(rolled.join("server").join("world.txt")).unwrap(),
        b"newest"
    );
    assert!(staged_in(&rolled).is_empty(), "{:?}", staged_in(&rolled));
}
