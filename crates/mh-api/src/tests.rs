use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use mh_app::sources::{
    BoxFut, CancellationToken, CatalogVersion, Checksum, JavaRuntime, ProgressFn, ResolvedServer,
    ServerJar,
};
use mh_app::{App, AppDeps, AppError, JavaSource, ServerCatalog};
use mh_core::{LoaderKind, McVersion};
use mh_platform::DataDirs;
use serde_json::{Value, json};
use tower::ServiceExt;

const PORT: u16 = 7878;
const HOST: &str = "127.0.0.1:7878";
const ORIGIN: &str = "http://127.0.0.1:7878";

struct FakeCatalog;

impl ServerCatalog for FakeCatalog {
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
                build: Some(232),
                java_major: 21,
                jar: ServerJar {
                    url: "https://example.invalid/paper.jar".to_owned(),
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
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            std::fs::write(dest, b"jar").unwrap();
            Ok(())
        })
    }
}

/// Test API không bật server thật, nên Java trỏ tới một file không tồn tại.
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
                version: "21".to_owned(),
            })
        })
    }
}

struct TestApi {
    _tmp: tempfile::TempDir,
    app: Arc<App>,
    router: axum::Router,
}

fn test_api() -> TestApi {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let deps = AppDeps {
        catalog: Arc::new(FakeCatalog),
        java: Arc::new(FakeJava),
        port_free: Arc::new(|_| true),
        free_space: Arc::new(|_| None),
        total_memory_mb: 8192,
        supervisor: mh_supervisor::SupervisorConfig::default(),
        save_flush_timeout: Duration::from_secs(60),
    };
    let app = Arc::new(App::with_deps(dirs, deps).unwrap());
    let router = crate::router(Arc::clone(&app), PORT);
    TestApi {
        _tmp: tmp,
        app,
        router,
    }
}

impl TestApi {
    /// Request giống trình duyệt mở giao diện MineHost: đúng Host, đúng Origin.
    async fn call(&self, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::HOST, HOST)
            .header(header::ORIGIN, ORIGIN);
        let body = match body {
            Some(json) => {
                req = req.header(header::CONTENT_TYPE, "application/json");
                Body::from(json.to_string())
            }
            None => Body::empty(),
        };
        send(self.router.clone(), req.body(body).unwrap()).await
    }

    async fn get(&self, uri: &str) -> (StatusCode, Value) {
        self.call(Method::GET, uri, None).await
    }

    async fn post(&self, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        self.call(Method::POST, uri, body).await
    }

    /// Tạo server và chờ Job cài xong.
    async fn create_server(&self, name: &str) -> String {
        let body = json!({ "name": name, "loader": "paper", "minecraft": "1.21.4" });
        let (status, created) = self.post("/api/v1/servers", Some(body)).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{created}");
        let job = created["job_id"].as_u64().unwrap();
        for _ in 0..200 {
            let (_, job) = self.get(&format!("/api/v1/jobs/{job}")).await;
            if job["state"] != "running" {
                assert_eq!(job["state"], "succeeded", "{job}");
                return created["server_id"].as_str().unwrap().to_owned();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("Job không xong");
    }
}

async fn send(router: axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let response = router.oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

fn raw(method: Method, uri: &str) -> axum::http::request::Builder {
    Request::builder().method(method).uri(uri)
}

#[tokio::test]
async fn health_reports_version() {
    let api = test_api();
    let (status, body) = api.get("/api/v1/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["version"], mh_app::VERSION);
}

#[tokio::test]
async fn system_reports_resources_and_suggestion() {
    let api = test_api();
    let (status, body) = api.get("/api/v1/system").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["cpu_threads"].as_u64().unwrap() > 0);
    let cpu = body["cpu_percent"].as_f64().unwrap();
    assert!((0.0..=100.0).contains(&cpu), "{cpu}");
    let used = body["memory_used_bytes"].as_u64().unwrap();
    assert!(used <= body["memory_total_bytes"].as_u64().unwrap());
    assert!(body.get("disk_free_bytes").is_some());
    assert_eq!(
        body["suggested_memory_mb"],
        json!({ "min": 1024, "max": 2048 })
    );
    assert!(body["lan_addresses"].is_array());
}

#[tokio::test]
async fn unknown_api_route_returns_error_code() {
    let api = test_api();
    let (status, body) = api.get("/api/v1/does-not-exist").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "NOT_FOUND");
}

#[tokio::test]
async fn non_api_routes_serve_html() {
    let api = test_api();
    let req = raw(Method::GET, "/servers/abc").header(header::HOST, HOST);
    let response = api
        .router
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers()["content-type"].to_str().unwrap();
    assert!(content_type.starts_with("text/html"));
}

#[tokio::test]
async fn foreign_host_is_forbidden() {
    let api = test_api();
    for host in ["evil.example:7878", "127.0.0.1:9999", "192.168.1.5:7878"] {
        let req = raw(Method::GET, "/api/v1/health").header(header::HOST, host);
        let (status, body) = send(api.router.clone(), req.body(Body::empty()).unwrap()).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{host}");
        assert_eq!(body["code"], "FORBIDDEN");
    }
    let req = raw(Method::GET, "/api/v1/health").header(header::HOST, "LOCALHOST:7878");
    let (status, _) = send(api.router.clone(), req.body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn post_needs_origin_or_minehost_header() {
    let api = test_api();
    let uri = "/api/v1/jobs/1/cancel";
    let bare = raw(Method::POST, uri).header(header::HOST, HOST);
    let (status, _) = send(api.router.clone(), bare.body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let wrong = raw(Method::POST, uri)
        .header(header::HOST, HOST)
        .header(header::ORIGIN, "https://evil.example")
        .header(crate::MINEHOST_HEADER, "1");
    let (status, _) = send(api.router.clone(), wrong.body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // CLI và test: không có Origin nhưng có `X-MineHost: 1` thì qua được lớp chặn.
    let cli = raw(Method::POST, uri)
        .header(header::HOST, HOST)
        .header(crate::MINEHOST_HEADER, "1");
    let (status, body) = send(api.router.clone(), cli.body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn form_encoded_body_is_rejected() {
    let api = test_api();
    let req = raw(Method::POST, "/api/v1/servers")
        .header(header::HOST, HOST)
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("name=x&loader=paper"))
        .unwrap();
    let (status, body) = send(api.router.clone(), req).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(body["code"], "INVALID_INPUT");
}

#[tokio::test]
async fn create_list_get_and_patch_server() {
    let api = test_api();
    let (_, list) = api.get("/api/v1/servers").await;
    assert_eq!(list, json!([]));

    let id = api.create_server("Server của tôi").await;
    let (status, list) = api.get("/api/v1/servers").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list[0]["id"], id);
    assert_eq!(list[0]["state"], "stopped");
    assert_eq!(list[0]["port"], 25565);

    let body =
        json!({ "name": "Đổi tên", "autostart": true, "memory_mb": { "min": 1024, "max": 3072 } });
    let (status, server) = api
        .call(Method::PATCH, &format!("/api/v1/servers/{id}"), Some(body))
        .await;
    assert_eq!(status, StatusCode::OK, "{server}");
    assert_eq!(server["name"], "Đổi tên");
    assert_eq!(server["autostart"], true);
    assert_eq!(server["memory_mb"]["max"], 3072);

    let (status, server) = api.get(&format!("/api/v1/servers/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(server["java_major"], 21);
}

#[tokio::test]
async fn create_rejects_bad_fields() {
    let api = test_api();
    for (body, field) in [
        (
            json!({ "name": " ", "loader": "paper", "minecraft": "1.21.4" }),
            "name",
        ),
        (
            json!({ "name": "a", "loader": "forge", "minecraft": "1.21.4" }),
            "loader",
        ),
        (
            json!({ "name": "a", "loader": "paper", "minecraft": "../x" }),
            "minecraft",
        ),
        (
            json!({ "name": "a", "loader": "paper", "minecraft": "1.21.4", "port": 80 }),
            "port",
        ),
    ] {
        let (status, err) = api.post("/api/v1/servers", Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["params"]["field"], field);
    }
    let (status, _) = api
        .post("/api/v1/servers", Some(json!({ "oops": 1 })))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn invalid_or_unknown_id() {
    let api = test_api();
    let (status, err) = api.get("/api/v1/servers/..%2Fsecret").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["params"]["field"], "id");
    let missing = mh_core::ServerId::generate();
    let (status, err) = api.get(&format!("/api/v1/servers/{missing}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(err["code"], "NOT_FOUND");
}

#[tokio::test]
async fn start_without_eula_is_refused_until_accepted() {
    let api = test_api();
    let id = api.create_server("A").await;
    let (status, err) = api
        .post(&format!("/api/v1/servers/{id}/actions/start"), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["code"], "EULA_NOT_ACCEPTED");

    let uri = format!("/api/v1/servers/{id}/eula");
    let (status, _) = api.post(&uri, Some(json!({ "accept": false }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, server) = api.post(&uri, Some(json!({ "accept": true }))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(server["eula_accepted"], true);

    let (status, err) = api
        .post(&format!("/api/v1/servers/{id}/actions/fly"), None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{err}");
    let (status, err) = api
        .post(&format!("/api/v1/servers/{id}/actions/stop"), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["code"], "SERVER_BUSY");
}

#[tokio::test]
async fn console_routes() {
    let api = test_api();
    let id = api.create_server("A").await;
    let (status, console) = api
        .get(&format!("/api/v1/servers/{id}/console?after_seq=0"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(console["lines"], json!([]));

    let uri = format!("/api/v1/servers/{id}/console");
    let (status, err) = api.post(&uri, Some(json!({ "command": "say hi" }))).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["code"], "SERVER_BUSY");
    let (status, _) = api.post(&uri, Some(json!({ "command": "  " }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn properties_round_trip() {
    let api = test_api();
    let id = api.create_server("Chào").await;
    let uri = format!("/api/v1/servers/{id}/properties");
    let (_, props) = api.get(&uri).await;
    assert!(
        props["properties"]
            .as_array()
            .unwrap()
            .contains(&json!({ "key": "motd", "value": "Chào" }))
    );

    let body = json!({ "properties": [{ "key": "pvp", "value": "false" }] });
    let (status, _) = api.call(Method::PUT, &uri, Some(body.clone())).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, props) = api.get(&uri).await;
    assert_eq!(props, body);
}

#[tokio::test]
async fn delete_needs_matching_name() {
    let api = test_api();
    let id = api.create_server("Thế giới").await;
    let base = format!("/api/v1/servers/{id}");
    let (status, _) = api
        .call(Method::DELETE, &format!("{base}?confirm_name=sai"), None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let name = "Th%E1%BA%BF%20gi%E1%BB%9Bi";
    let (status, body) = api
        .call(Method::DELETE, &format!("{base}?confirm_name={name}"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["trash_path"].as_str().unwrap().contains("trash"));
    let (status, _) = api.get(&base).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn jobs_and_catalog() {
    let api = test_api();
    let (status, versions) = api.get("/api/v1/catalog/paper/versions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(versions[0]["id"], "1.21.4");
    assert_eq!(versions[0]["release_time"], "2024-12-03T10:12:00Z");
    let (status, empty) = api.get("/api/v1/jobs").await;
    assert_eq!((status, empty), (StatusCode::OK, json!([])));
    let (status, err) = api.get("/api/v1/jobs?server_id=not-a-ulid").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["code"], "INVALID_INPUT");
    let (status, _) = api.get("/api/v1/jobs/999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = api.post("/api/v1/jobs/999/cancel", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let mk = |name: &str| json!({ "name": name, "loader": "paper", "minecraft": "1.21.4" });
    let (_, first) = api.post("/api/v1/servers", Some(mk("A"))).await;
    let (_, second) = api.post("/api/v1/servers", Some(mk("B"))).await;
    let job = second["job_id"].as_u64().unwrap();
    let (status, job_dto) = api.get(&format!("/api/v1/jobs/{job}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(job_dto["kind"], "create_server");
    assert!(job_dto["started_at"].as_str().unwrap().contains('T'));
    assert!(job_dto.get("finished_at").is_some());

    let (status, listed) = api.get("/api/v1/jobs").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed[0]["id"], second["job_id"]);
    assert_eq!(listed[1]["id"], first["job_id"]);
    let id = first["server_id"].as_str().unwrap();
    let (status, only) = api.get(&format!("/api/v1/jobs?server_id={id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        only.as_array()
            .is_some_and(|rows| rows.len() == 1 && rows[0]["server_id"] == id)
    );

    let (status, _) = api.post(&format!("/api/v1/jobs/{job}/cancel"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn shutdown_route_signals_the_binary() {
    let api = test_api();
    let (status, _) = api.post("/api/v1/app/shutdown", None).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let signalled = tokio::time::timeout(Duration::from_secs(1), api.app.shutdown_requested());
    assert!(signalled.await.is_ok());
}

#[test]
fn openapi_lists_all_routes() {
    let spec = crate::openapi();
    for path in [
        "/api/v1/health",
        "/api/v1/system",
        "/api/v1/servers",
        "/api/v1/servers/{id}",
        "/api/v1/servers/{id}/eula",
        "/api/v1/servers/{id}/actions/{action}",
        "/api/v1/servers/{id}/console",
        "/api/v1/servers/{id}/properties",
        "/api/v1/catalog/paper/versions",
        "/api/v1/jobs",
        "/api/v1/jobs/{id}",
        "/api/v1/jobs/{id}/cancel",
        "/api/v1/app/shutdown",
    ] {
        assert!(spec.paths.paths.contains_key(path), "thiếu {path}");
    }
}
