//! Đổi phiên bản và cập nhật build (spec P3, mục 3.9): có bản `before_update`, không có gì mới
//! thì không tắt server và không sao lưu, thiếu xác nhận bị từ chối.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use mh_app::sources::{
    BoxFut, CatalogVersion, InstallPlan, InstallerRun, LoaderInstall, PlannedFile, ProgressFn,
    ResolvedServer,
};
use mh_app::{
    App, AppDeps, AppError, BackupKind, CreateRequest, JavaSource, JobSnapshot, JobState,
    MetricsConfig, ServerCatalog, VersionChange,
};
use mh_core::instance::JarName;
use mh_core::{ErrorCode, Launch, LoaderKind, LoaderVersion, McVersion, ServerId, ServerName};
use mh_java::JavaRuntime;
use mh_net::{CancellationToken, Checksum, hosts};
use mh_platform::DataDirs;
use mh_providers::ServerJar;
use mh_supervisor::SupervisorConfig;

const WAIT: Duration = Duration::from_secs(20);

/// Paper: build tăng khi test đổi `build`. Fabric: installer `fake-mc` ghi jar khởi động.
struct FakeCatalog {
    build: AtomicU32,
}

impl ServerCatalog for FakeCatalog {
    fn versions(&self, _: LoaderKind) -> BoxFut<'_, Result<Vec<CatalogVersion>, AppError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn resolve<'a>(
        &'a self,
        _: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<ResolvedServer, AppError>> {
        let build = self.build.load(Ordering::SeqCst);
        Box::pin(async move {
            Ok(ResolvedServer {
                minecraft: minecraft.clone(),
                build: Some(build),
                java_major: 21,
                jar: ServerJar {
                    url: format!("https://example.invalid/paper-{build}.jar"),
                    checksum: Checksum::sha256(&"0".repeat(64)).unwrap(),
                    size: 8,
                },
            })
        })
    }

    fn download<'a>(
        &'a self,
        jar: &'a ServerJar,
        dest: &'a Path,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            std::fs::write(dest, jar.url.as_bytes()).unwrap();
            Ok(())
        })
    }

    fn plan<'a>(
        &'a self,
        _: LoaderKind,
        minecraft: &'a McVersion,
        wanted: Option<&'a LoaderVersion>,
    ) -> BoxFut<'a, Result<LoaderInstall, AppError>> {
        let jar = ".minehost-install/fake-installer.jar".to_owned();
        let args = ["--install", "--write", "fabric-server-launch.jar"];
        let plan = InstallPlan {
            minecraft: minecraft.clone(),
            loader_version: wanted
                .cloned()
                .unwrap_or_else(|| LoaderVersion::parse("0.16.9").unwrap()),
            files: vec![PlannedFile {
                url: "https://maven.fabricmc.net/fake-installer.jar".to_owned(),
                checksum: Checksum::sha256(&"0".repeat(64)).unwrap(),
                size: None,
                dest: jar.clone(),
                hosts: hosts::LOADERS,
            }],
            installer: Some(InstallerRun {
                jar,
                args: args.iter().map(|a| (*a).to_owned()).collect(),
            }),
            launch: Launch::Jar(JarName::parse("fabric-server-launch.jar").unwrap()),
        };
        Box::pin(async move {
            Ok(LoaderInstall {
                plan,
                java_major: 21,
            })
        })
    }

    fn download_file<'a>(
        &'a self,
        _: &'a PlannedFile,
        dest: &'a Path,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::write(dest, b"fake installer").unwrap();
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
    _tmp: tempfile::TempDir,
    app: Arc<App>,
    catalog: Arc<FakeCatalog>,
    root: PathBuf,
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let catalog = Arc::new(FakeCatalog {
        build: AtomicU32::new(100),
    });
    let deps = AppDeps {
        catalog: Arc::clone(&catalog) as Arc<dyn ServerCatalog>,
        java: Arc::new(FakeJava),
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
    let root = tmp.path().join("servers");
    Env {
        app: Arc::new(App::with_deps(dirs, deps).unwrap()),
        _tmp: tmp,
        catalog,
        root,
    }
}

async fn finish(app: &App, job: u64) -> JobSnapshot {
    let mut rx = app.jobs().subscribe(job).unwrap();
    tokio::time::timeout(WAIT, rx.wait_for(|s| s.state != JobState::Running))
        .await
        .unwrap()
        .unwrap()
        .clone()
}

async fn create(app: &Arc<App>, loader: LoaderKind) -> ServerId {
    let created = app
        .create(CreateRequest {
            name: ServerName::parse("Đổi bản").unwrap(),
            loader,
            minecraft: McVersion::parse("1.21.4").unwrap(),
            loader_version: None,
            memory_mb: None,
            port: None,
        })
        .unwrap();
    let snap = finish(app, created.job_id).await;
    assert_eq!(snap.state, JobState::Succeeded, "{snap:?}");
    created.server_id
}

fn change(minecraft: Option<&str>, loader_version: Option<&str>) -> VersionChange {
    VersionChange {
        minecraft: minecraft.map(|v| McVersion::parse(v).unwrap()),
        loader_version: loader_version.map(|v| LoaderVersion::parse(v).unwrap()),
        confirm: true,
    }
}

async fn run(env: &Env, id: ServerId, req: VersionChange) -> serde_json::Value {
    let job = env.app.change_version(id, req).unwrap();
    let snap = finish(&env.app, job).await;
    assert_eq!(snap.state, JobState::Succeeded, "{snap:?}");
    snap.result.unwrap()
}

#[tokio::test]
async fn paper_build_update_backs_up_and_replaces_the_jar() {
    let env = env();
    let id = create(&env.app, LoaderKind::Paper).await;
    env.catalog.build.store(101, Ordering::SeqCst);

    let result = run(&env, id, change(None, None)).await;
    assert_eq!(result["changed"], true, "{result}");
    assert_eq!(result["build"], 101);
    let spec = env.app.get(id).unwrap().spec.unwrap();
    assert_eq!(spec.game.build, Some(101));
    let jar = env.root.join(id.to_string()).join("server/server.jar");
    assert_eq!(
        std::fs::read_to_string(jar).unwrap(),
        "https://example.invalid/paper-101.jar"
    );
    let backups = env.app.list_backups(id).unwrap();
    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].kind, BackupKind::BeforeUpdate);
    assert_eq!(result["backup_id"], backups[0].id.as_str());
}

#[tokio::test]
async fn nothing_new_means_no_backup() {
    let env = env();
    let id = create(&env.app, LoaderKind::Paper).await;
    let result = run(&env, id, change(None, None)).await;
    assert_eq!(result["changed"], false, "{result}");
    assert!(env.app.list_backups(id).unwrap().is_empty());
}

#[tokio::test]
async fn minecraft_change_on_paper_updates_the_game_section() {
    let env = env();
    let id = create(&env.app, LoaderKind::Paper).await;
    let result = run(&env, id, change(Some("1.21.1"), None)).await;
    assert_eq!(result["minecraft"], "1.21.1", "{result}");
    let spec = env.app.get(id).unwrap().spec.unwrap();
    assert_eq!(spec.game.minecraft.as_str(), "1.21.1");
}

#[tokio::test]
async fn fabric_loader_version_change_runs_the_installer_again() {
    let env = env();
    let id = create(&env.app, LoaderKind::Fabric).await;
    let before = env.app.get(id).unwrap().spec.unwrap();
    assert_eq!(before.game.loader_version.unwrap().as_str(), "0.16.9");

    let result = run(&env, id, change(None, Some("0.16.10"))).await;
    assert_eq!(result["loader_version"], "0.16.10", "{result}");
    let spec = env.app.get(id).unwrap().spec.unwrap();
    assert_eq!(spec.game.loader_version.unwrap().as_str(), "0.16.10");
    assert_eq!(spec.game.jar.as_str(), "fabric-server-launch.jar");
    assert_eq!(env.app.list_backups(id).unwrap().len(), 1);
}

#[tokio::test]
async fn missing_confirm_or_loader_version_on_paper_is_rejected() {
    let env = env();
    let id = create(&env.app, LoaderKind::Paper).await;
    let mut req = change(Some("1.20.4"), None);
    req.confirm = false;
    let err = env.app.change_version(id, req).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert_eq!(err.params["field"], "confirm");

    let err = env
        .app
        .change_version(id, change(None, Some("0.16.9")))
        .unwrap_err();
    assert_eq!(err.params["field"], "loader_version");
}
