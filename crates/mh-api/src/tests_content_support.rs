//! Catalog server giả và helper gọi API cho `tests_content`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use mh_app::sources::{
    BoxFut, CancellationToken, CatalogVersion, Checksum, HostAllowlist, InstallPlan, JavaRuntime,
    LoaderInstall, LoaderRelease, PlannedFile, ProgressFn, ResolvedServer, ServerJar,
};
use mh_app::{App, AppDeps, AppError, JavaSource, ServerCatalog};
use mh_core::{ErrorCode, Launch, LoaderKind, LoaderVersion, McVersion, instance::JarName};
use mh_platform::DataDirs;
use serde_json::{Value, json};
use tokio::sync::Notify;

use super::super::{HOST, ORIGIN, PORT, TestApi, send};
use super::catalog::FakeModCatalog;

pub(super) struct ContentServerCatalog {
    pub(super) hold_download: Option<Arc<Notify>>,
}

impl ServerCatalog for ContentServerCatalog {
    fn versions(&self, _: LoaderKind) -> BoxFut<'_, Result<Vec<CatalogVersion>, AppError>> {
        Box::pin(async {
            Ok(vec![CatalogVersion {
                id: McVersion::parse("1.21.4").unwrap(),
                release_time: time::macros::datetime!(2024-12-03 10:12 UTC),
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
                build: Some(1),
                java_major: 21,
                jar: ServerJar {
                    url: "https://example.invalid/server.jar".to_owned(),
                    checksum: Checksum::sha256(&"0".repeat(64)).unwrap(),
                    size: 1,
                },
            })
        })
    }

    fn download<'a>(
        &'a self,
        _: &'a ServerJar,
        dest: &'a Path,
        _: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        let hold = self.hold_download.clone();
        Box::pin(async move {
            if let Some(notify) = hold {
                tokio::select! {
                    () = notify.notified() => {}
                    () = cancel.cancelled() => return Err(AppError::new(ErrorCode::JobCancelled, "hủy")),
                }
            }
            std::fs::write(dest, b"jar").unwrap();
            Ok(())
        })
    }

    fn loader_versions<'a>(
        &'a self,
        _: LoaderKind,
        _: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<LoaderRelease>, AppError>> {
        Box::pin(async {
            Ok(vec![LoaderRelease {
                version: LoaderVersion::parse("0.16.9").unwrap(),
                stable: true,
            }])
        })
    }

    fn plan<'a>(
        &'a self,
        loader: LoaderKind,
        minecraft: &'a McVersion,
        wanted: Option<&'a LoaderVersion>,
    ) -> BoxFut<'a, Result<LoaderInstall, AppError>> {
        let ver = wanted
            .cloned()
            .unwrap_or_else(|| LoaderVersion::parse("0.16.9").unwrap());
        Box::pin(async move {
            if loader != LoaderKind::Fabric {
                return Err(AppError::new(ErrorCode::InvalidInput, "chỉ giả lập fabric"));
            }
            let launch_jar = "fabric-server-launch.jar";
            Ok(LoaderInstall {
                java_major: 21,
                plan: InstallPlan {
                    minecraft: minecraft.clone(),
                    loader_version: ver,
                    files: vec![PlannedFile {
                        url: "https://example.invalid/fabric-launch.jar".to_owned(),
                        checksum: Checksum::sha256(&"0".repeat(64)).unwrap(),
                        size: Some(3),
                        dest: launch_jar.to_owned(),
                        hosts: HostAllowlist::new(&[]),
                    }],
                    installer: None,
                    launch: Launch::Jar(JarName::parse(launch_jar).unwrap()),
                },
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
            std::fs::write(dest, b"jar").unwrap();
            Ok(())
        })
    }
}

pub(super) struct FakeJava;

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
                version: "21".to_owned(),
            })
        })
    }
}

pub(super) fn content_api_with_cf(hold_download: Option<Arc<Notify>>, with_cf: bool) -> TestApi {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let deps = AppDeps {
        catalog: Arc::new(ContentServerCatalog { hold_download }),
        java: Arc::new(FakeJava),
        port_free: Arc::new(|_| true),
        free_space: Arc::new(|_| None),
        total_memory_mb: 8192,
        supervisor: mh_supervisor::SupervisorConfig::default(),
        save_flush_timeout: Duration::from_secs(60),
        metrics: mh_app::MetricsConfig::default(),
    };
    let mut builder = App::with_deps(dirs, deps)
        .unwrap()
        .with_content(Arc::new(FakeModCatalog::sample()))
        .with_curseforge(Arc::new(FakeModCatalog::curseforge()));
    if with_cf {
        builder = builder.with_curseforge_key(mh_core::CurseforgeKey::parse("test-key").unwrap());
    }
    let app = Arc::new(builder);
    let router = crate::router(Arc::clone(&app), PORT);
    TestApi {
        _tmp: tmp,
        app,
        router,
    }
}

pub(super) fn content_api(hold_download: Option<Arc<Notify>>) -> TestApi {
    content_api_with_cf(hold_download, false)
}

pub(super) async fn create_with_loader(
    api: &TestApi,
    name: &str,
    loader: &str,
    port: u16,
) -> String {
    let body = json!({ "name": name, "loader": loader, "minecraft": "1.21.4", "port": port });
    let (status, created) = api.post("/api/v1/servers", Some(body)).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{created}");
    let job = created["job_id"].as_u64().unwrap();
    wait_job(api, job).await;
    created["server_id"].as_str().unwrap().to_owned()
}

pub(super) async fn wait_job(api: &TestApi, job_id: u64) {
    for _ in 0..200 {
        let (_, job) = api.get(&format!("/api/v1/jobs/{job_id}")).await;
        if job["state"] != "running" {
            assert_eq!(job["state"], "succeeded", "{job}");
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Job không xong");
}

pub(super) async fn upload_jar(
    router: &axum::Router,
    uri: &str,
    name: &str,
    bytes: &[u8],
) -> (StatusCode, Value) {
    let boundary = "ContentBoundary";
    let mut body = Vec::new();
    body.extend(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\n\r\n"
        )
        .into_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend(format!("\r\n--{boundary}--\r\n").into_bytes());
    let req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::HOST, HOST)
        .header(header::ORIGIN, ORIGIN)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header(header::CONTENT_LENGTH, body.len().to_string())
        .body(Body::from(body))
        .unwrap();
    send(router.clone(), req).await
}
