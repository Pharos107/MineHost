//! Job tạo server có mod loader (spec P3, mục 3.1) với catalog giả và `fake-mc` làm installer.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mh_app::sources::{
    BoxFut, CatalogVersion, InstallPlan, InstallerRun, LoaderInstall, PlannedFile, ProgressFn,
    ResolvedServer,
};
use mh_app::{
    App, AppDeps, AppError, CreateRequest, JavaSource, JobState, MetricsConfig, ServerCatalog,
};
use mh_core::instance::{JarName, LaunchMode};
use mh_core::{
    ArgsFilePath, ErrorCode, Launch, LoaderKind, LoaderVersion, McVersion, ServerId, ServerName,
    ServerState,
};
use mh_java::JavaRuntime;
use mh_net::{CancellationToken, Checksum, hosts};
use mh_platform::DataDirs;
use mh_providers::ServerJar;
use mh_supervisor::SupervisorConfig;

const WAIT: Duration = Duration::from_secs(20);

/// Installer giả: các đối số `fake-mc --install` cộng file cần tạo, cách chạy dự kiến.
struct FakeCatalog {
    installer_args: Vec<&'static str>,
    launch: Launch,
}

impl FakeCatalog {
    fn fabric(exit_code: &'static str) -> Self {
        Self {
            installer_args: vec![
                "--install",
                "--write",
                "fabric-server-launch.jar",
                "--exit-code",
                exit_code,
            ],
            launch: Launch::Jar(JarName::parse("fabric-server-launch.jar").unwrap()),
        }
    }

    fn neoforge(write: &'static [&'static str]) -> Self {
        let mut installer_args = vec!["--install"];
        for file in write {
            installer_args.extend(["--write", file]);
        }
        Self {
            installer_args,
            launch: Launch::ArgsFile(ArgsFilePath::parse("libraries/neo/{os}_args.txt").unwrap()),
        }
    }
}

impl ServerCatalog for FakeCatalog {
    fn versions(&self, _: LoaderKind) -> BoxFut<'_, Result<Vec<CatalogVersion>, AppError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn resolve<'a>(
        &'a self,
        _: LoaderKind,
        _: &'a McVersion,
    ) -> BoxFut<'a, Result<ResolvedServer, AppError>> {
        Box::pin(async { panic!("server có loader không đi qua resolve") })
    }

    fn download<'a>(
        &'a self,
        _: &'a ServerJar,
        _: &'a Path,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async { panic!("server có loader không đi qua download") })
    }

    fn plan<'a>(
        &'a self,
        _: LoaderKind,
        minecraft: &'a McVersion,
        wanted: Option<&'a LoaderVersion>,
    ) -> BoxFut<'a, Result<LoaderInstall, AppError>> {
        let jar = ".minehost-install/fake-installer.jar".to_owned();
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
                args: self.installer_args.iter().map(|a| (*a).to_owned()).collect(),
            }),
            launch: self.launch.clone(),
        };
        Box::pin(async move { Ok(LoaderInstall { plan, java_major: 21 }) })
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

fn test_app(catalog: FakeCatalog) -> (tempfile::TempDir, Arc<App>) {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let deps = AppDeps {
        catalog: Arc::new(catalog),
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
    (tmp, Arc::new(App::with_deps(dirs, deps).unwrap()))
}

fn request(loader: LoaderKind) -> CreateRequest {
    CreateRequest {
        name: ServerName::parse("Có mod").unwrap(),
        loader,
        minecraft: McVersion::parse("1.21.4").unwrap(),
        loader_version: None,
        memory_mb: None,
        port: None,
    }
}

async fn run(app: &Arc<App>, req: CreateRequest) -> (ServerId, JobState) {
    let created = app.create(req).unwrap();
    let mut rx = app.jobs().subscribe(created.job_id).unwrap();
    let snap = tokio::time::timeout(WAIT, rx.wait_for(|s| s.state != JobState::Running))
        .await
        .unwrap()
        .unwrap()
        .clone();
    (created.server_id, snap.state)
}

fn server_dir(tmp: &tempfile::TempDir, id: ServerId) -> PathBuf {
    tmp.path().join("servers").join(id.to_string()).join("server")
}

#[tokio::test]
async fn fabric_server_is_installed_with_launcher_jar() {
    let (tmp, app) = test_app(FakeCatalog::fabric("0"));
    let mut req = request(LoaderKind::Fabric);
    req.loader_version = Some(LoaderVersion::parse("0.16.10").unwrap());
    let (id, state) = run(&app, req).await;
    assert_eq!(state, JobState::Succeeded);

    let spec = app.get(id).unwrap().spec.unwrap();
    assert_eq!(spec.game.loader, LoaderKind::Fabric);
    assert_eq!(spec.game.loader_version.unwrap().as_str(), "0.16.10");
    assert_eq!(spec.game.jar.as_str(), "fabric-server-launch.jar");
    assert!(spec.game.launch.is_none());
    let dir = server_dir(&tmp, id);
    assert!(dir.join("fabric-server-launch.jar").is_file());
    assert!(!dir.join(".minehost-install").exists(), "installer tạm phải bị xóa");
}

#[tokio::test]
async fn failed_installer_reports_code_and_cleans_up() {
    let (tmp, app) = test_app(FakeCatalog::fabric("3"));
    let (id, state) = run(&app, request(LoaderKind::Fabric)).await;
    let JobState::Failed { code, params } = state else {
        panic!("Job phải lỗi: {state:?}");
    };
    assert_eq!(code, ErrorCode::LoaderInstallFailed);
    assert_eq!(params["exit_code"], 3);
    let tail = params["log_tail"].as_array().unwrap();
    assert!(
        tail.iter().any(|l| l.as_str().unwrap().contains("finished with code 3")),
        "{tail:?}"
    );
    assert!(!tmp.path().join("servers").join(id.to_string()).exists());
}

#[tokio::test]
async fn missing_args_file_fails_the_job() {
    let (_tmp, app) = test_app(FakeCatalog::neoforge(&["libraries/neo/other.txt"]));
    let (_id, state) = run(&app, request(LoaderKind::NeoForge)).await;
    let JobState::Failed { code, params } = state else {
        panic!("Job phải lỗi: {state:?}");
    };
    assert_eq!(code, ErrorCode::LoaderInstallFailed);
    assert!(params["missing"].as_str().unwrap().ends_with("_args.txt"));
}

/// Server chạy bằng `@file` tới trạng thái Running, rồi tắt sạch.
#[tokio::test]
async fn neoforge_server_starts_from_args_file() {
    let files = &["libraries/neo/unix_args.txt", "libraries/neo/win_args.txt"];
    let (_tmp, app) = test_app(FakeCatalog::neoforge(files));
    let (id, state) = run(&app, request(LoaderKind::NeoForge)).await;
    assert_eq!(state, JobState::Succeeded);
    let spec = app.get(id).unwrap().spec.unwrap();
    let Some(LaunchMode::ArgsFile(path)) = spec.game.launch.map(|l| l.mode) else {
        panic!("phải có [game.launch] kiểu args_file");
    };
    assert_eq!(path.as_str(), "libraries/neo/{os}_args.txt");

    app.accept_eula(id).unwrap();
    app.start(id).await.unwrap();
    let handle = app.handle(id).unwrap();
    let status = handle.wait_for(WAIT, |s| s.state == ServerState::Running).await;
    assert_eq!(status.state, ServerState::Running, "{status:?}");
    app.stop(id).await.unwrap();
    let status = handle.wait_for(WAIT, |s| s.state == ServerState::Stopped).await;
    assert_eq!(status.state, ServerState::Stopped, "{status:?}");
}
