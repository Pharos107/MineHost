//! Job sao lưu với `fake-mc`: thứ tự lệnh save, file mô tả, và `save-on` khi lỗi.

// Hàm trợ giúp ngoài `#[test]` không được clippy coi là code test.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use mh_app::jobs::JobStep;
use mh_app::sources::{BoxFut, CatalogVersion, ProgressFn, ResolvedServer};
use mh_app::{
    App, AppDeps, AppError, BackupKind, BackupMeta, BackupRequest, CreateRequest, JavaSource,
    JobState, ServerCatalog,
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
use sha2::{Digest, Sha256};
use time::macros::datetime;

const WAIT: Duration = Duration::from_secs(10);

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
    free: u64,
    save_flush: Duration,
}

impl Env {
    fn new() -> Self {
        Self {
            tmp: tempfile::tempdir().unwrap(),
            free: 100 * 1024 * 1024 * 1024,
            save_flush: Duration::from_secs(60),
        }
    }

    fn app(&self) -> Arc<App> {
        let dirs = DataDirs::resolve(Some(self.tmp.path().to_path_buf())).unwrap();
        dirs.ensure().unwrap();
        // `0` chỉ áp vào `backups/`. Job tạo server đo thư mục gốc và cần chỗ trống riêng.
        let backup_free = self.free;
        let root_free = if backup_free == 0 {
            100_u64 << 30
        } else {
            backup_free
        };
        let deps = AppDeps {
            catalog: Arc::new(FakeCatalog),
            java: Arc::new(FakeJava),
            port_free: Arc::new(|_| true),
            free_space: Arc::new(move |path| {
                let bytes = if path.ends_with("backups") {
                    backup_free
                } else {
                    root_free
                };
                Some(bytes)
            }),
            total_memory_mb: 16 * 1024,
            supervisor: SupervisorConfig {
                stop_timeout: Duration::from_millis(500),
                ..SupervisorConfig::default()
            },
            save_flush_timeout: self.save_flush,
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
        loader_version: None,
        memory_mb: None,
        port: None,
    }
}

fn manual(note: Option<&str>, protected: bool) -> BackupRequest {
    BackupRequest {
        note: note.map(str::to_owned),
        protected,
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

async fn boot(env: &Env, app: &App, spec: &InstanceSpec, hang_save: bool) {
    app.accept_eula(spec.id).unwrap();
    if hang_save {
        let path = env.server_dir(spec).parent().unwrap().join("instance.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        let mut parsed = InstanceSpec::from_toml(&text).unwrap();
        parsed.runtime.jvm_args = vec!["--hang-on-save-all".to_owned()];
        std::fs::write(&path, parsed.to_toml().unwrap()).unwrap();
    }
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

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assert_save_on_closes(commands: &[String]) {
    let off = commands.iter().position(|line| line == "save-off").unwrap();
    let on = commands.iter().position(|line| line == "save-on").unwrap();
    assert!(off < on, "{commands:?}");
    if let Some(flush) = commands.iter().position(|line| line == "save-all flush") {
        assert!(off < flush && flush < on, "{commands:?}");
    }
}

#[tokio::test]
async fn running_backup_saves_in_order_and_writes_descriptor() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app, "A").await;
    boot(&env, &app, &spec, false).await;

    let started = app
        .backup(spec.id, manual(Some("trước khi sửa"), false))
        .unwrap();
    assert_eq!(finish(&app, started.job_id).await, JobState::Succeeded);
    assert_eq!(
        stdin_commands(&app, spec.id),
        ["save-off", "save-all flush", "save-on"]
    );

    let snap = app.jobs().get(started.job_id).unwrap();
    let progress = snap.progress.unwrap();
    assert!(matches!(progress.step, JobStep::Compress));
    assert!(progress.bytes_done.is_some());

    let listed = app.list_backups(spec.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_descriptor(&env, &spec, &listed[0], "trước khi sửa", false);
    app.kill(spec.id).await.unwrap();
}

#[tokio::test]
async fn save_on_still_sent_when_compress_fails() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app, "A").await;
    boot(&env, &app, &spec, false).await;
    // Thư mục đích là một file: `create_dir_all` lỗi sau khi đã gửi save-off.
    std::fs::write(env.backup_dir(spec.id), b"not-a-directory").unwrap();

    let started = app.backup(spec.id, manual(None, false)).unwrap();
    let state = finish(&app, started.job_id).await;
    assert!(
        matches!(
            state,
            JobState::Failed {
                code: ErrorCode::Internal,
                ..
            }
        ),
        "{state:?}"
    );
    assert_eq!(
        stdin_commands(&app, spec.id),
        ["save-off", "save-all flush", "save-on"]
    );
    app.kill(spec.id).await.unwrap();
}

#[tokio::test]
async fn save_on_still_sent_when_job_is_cancelled() {
    let mut env = Env::new();
    env.save_flush = Duration::from_secs(2);
    let app = env.app();
    let spec = create(&app, "A").await;
    boot(&env, &app, &spec, true).await;

    let started = app.backup(spec.id, manual(None, false)).unwrap();
    tokio::time::timeout(WAIT, async {
        loop {
            if stdin_commands(&app, spec.id)
                .iter()
                .any(|line| line == "save-off")
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    app.jobs().cancel(started.job_id).unwrap();
    assert_eq!(finish(&app, started.job_id).await, JobState::Cancelled);
    assert_save_on_closes(&stdin_commands(&app, spec.id));
    app.kill(spec.id).await.unwrap();
}

#[tokio::test]
async fn stopped_server_backup_sends_no_commands() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app, "A").await;

    let started = app.backup(spec.id, manual(Some("tắt"), false)).unwrap();
    assert_eq!(finish(&app, started.job_id).await, JobState::Succeeded);
    assert!(stdin_commands(&app, spec.id).is_empty());
    let listed = app.list_backups(spec.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].note.as_deref(), Some("tắt"));
    assert!(
        env.backup_dir(spec.id)
            .join(format!("{}.tar.zst", listed[0].id))
            .is_file()
    );
}

#[tokio::test]
async fn save_all_timeout_fails_and_sends_save_on() {
    let mut env = Env::new();
    env.save_flush = Duration::from_millis(200);
    let app = env.app();
    let spec = create(&app, "A").await;
    boot(&env, &app, &spec, true).await;

    let started_at = Instant::now();
    let started = app.backup(spec.id, manual(None, false)).unwrap();
    let state = finish(&app, started.job_id).await;
    assert!(
        started_at.elapsed() < Duration::from_secs(5),
        "hạn chờ không được rút"
    );
    assert!(
        matches!(
            state,
            JobState::Failed {
                code: ErrorCode::Internal,
                ..
            }
        ),
        "{state:?}"
    );
    assert_eq!(
        stdin_commands(&app, spec.id),
        ["save-off", "save-all flush", "save-on"]
    );
    app.kill(spec.id).await.unwrap();
}

#[tokio::test]
async fn protected_backup_cannot_be_deleted_and_list_is_newest_first() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app, "A").await;

    let older = app.backup(spec.id, manual(Some("cũ"), false)).unwrap();
    assert_eq!(finish(&app, older.job_id).await, JobState::Succeeded);
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let newer = app.backup(spec.id, manual(Some("mới"), true)).unwrap();
    assert_eq!(finish(&app, newer.job_id).await, JobState::Succeeded);

    std::fs::write(env.backup_dir(spec.id).join("broken.json"), b"oops").unwrap();
    let listed = app.list_backups(spec.id).unwrap();
    assert_eq!(listed.len(), 2, "file mô tả hỏng phải bị bỏ qua");
    assert_eq!(listed[0].note.as_deref(), Some("mới"));
    assert_eq!(listed[1].note.as_deref(), Some("cũ"));
    assert!(listed[0].created_at >= listed[1].created_at);

    let id = &listed[0].id;
    let err = app.delete_backup(spec.id, id).unwrap_err();
    assert_eq!(err.code, ErrorCode::BackupProtected);
    assert!(
        env.backup_dir(spec.id)
            .join(format!("{id}.tar.zst"))
            .is_file()
    );
    assert!(env.backup_dir(spec.id).join(format!("{id}.json")).is_file());

    let updated = app
        .update_backup(spec.id, id, Some(Some("đã sửa".to_owned())), Some(false))
        .unwrap();
    assert_eq!(updated.note.as_deref(), Some("đã sửa"));
    assert!(!updated.protected);
    app.delete_backup(spec.id, id).unwrap();
    assert!(!env.backup_dir(spec.id).join(format!("{id}.json")).exists());
    assert!(
        !env.backup_dir(spec.id)
            .join(format!("{id}.tar.zst"))
            .exists()
    );
    assert_eq!(app.list_backups(spec.id).unwrap().len(), 1);
}

#[tokio::test]
async fn startup_and_reconcile_remove_incomplete_archives() {
    let env = Env::new();
    let server = env.tmp.path().join("backups").join("server-a");
    std::fs::create_dir_all(&server).unwrap();
    std::fs::write(server.join("keep.tar.zst"), b"keep").unwrap();
    std::fs::write(server.join("keep.json"), b"{}").unwrap();
    std::fs::write(server.join("orphan.tar.zst"), b"orphan").unwrap();
    std::fs::write(server.join("half.tar.zst.part"), b"part").unwrap();

    let app = env.app();
    assert!(server.join("keep.tar.zst").is_file());
    assert!(server.join("keep.json").is_file());
    assert!(!server.join("orphan.tar.zst").exists());
    assert!(!server.join("half.tar.zst.part").exists());

    std::fs::write(server.join("later.tar.zst"), b"later").unwrap();
    std::fs::write(server.join("later.tar.zst.part"), b"part").unwrap();
    assert_eq!(app.reconcile().await.unwrap(), 0);
    assert!(!server.join("later.tar.zst").exists());
    assert!(!server.join("later.tar.zst.part").exists());
    assert!(server.join("keep.tar.zst").is_file());
}

#[tokio::test]
async fn not_enough_disk_fails_before_save_commands() {
    let mut env = Env::new();
    env.free = 0;
    let app = env.app();
    let spec = create(&app, "A").await;
    let started = app.backup(spec.id, manual(None, false)).unwrap();
    let state = finish(&app, started.job_id).await;
    assert!(
        matches!(
            state,
            JobState::Failed {
                code: ErrorCode::NotEnoughDisk,
                ..
            }
        ),
        "{state:?}"
    );
    assert!(stdin_commands(&app, spec.id).is_empty());
    assert!(app.list_backups(spec.id).unwrap().is_empty());
}

fn assert_descriptor(
    env: &Env,
    spec: &InstanceSpec,
    meta: &BackupMeta,
    note: &str,
    protected: bool,
) {
    let dir = env.backup_dir(spec.id);
    let archive = dir.join(format!("{}.tar.zst", meta.id));
    let json_path = dir.join(format!("{}.json", meta.id));
    let bytes = std::fs::read(&archive).unwrap();
    let raw: Value = serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();

    assert_eq!(meta.server_id, spec.id);
    assert_eq!(meta.kind, BackupKind::Manual);
    assert_eq!(meta.minecraft, "1.21.4");
    assert_eq!(meta.loader, LoaderKind::Paper);
    assert_eq!(meta.note.as_deref(), Some(note));
    assert_eq!(meta.protected, protected);
    assert_eq!(meta.size, bytes.len() as u64);
    assert_eq!(meta.sha256, sha256_hex(&bytes));
    assert_eq!(raw["id"], meta.id);
    assert_eq!(raw["server_id"], spec.id.to_string());
    assert_eq!(raw["kind"], "manual");
    assert_eq!(raw["size"], meta.size);
    assert_eq!(raw["sha256"], meta.sha256);
    assert_eq!(raw["minecraft"], "1.21.4");
    assert_eq!(raw["loader"], "paper");
    assert_eq!(raw["note"], note);
    assert_eq!(raw["protected"], protected);
    assert!(raw["created_at"].as_str().unwrap().ends_with('Z'));
}
