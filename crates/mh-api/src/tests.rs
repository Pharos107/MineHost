use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use mh_app::App;
use mh_platform::DataDirs;
use tower::ServiceExt;

fn test_app() -> (tempfile::TempDir, axum::Router) {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    (tmp, crate::router(Arc::new(App::new(dirs))))
}

async fn get_json(router: axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let request = Request::get(uri).body(Body::empty()).unwrap();
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn health_reports_version() {
    let (_tmp, router) = test_app();
    let (status, body) = get_json(router, "/api/v1/health").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["version"], mh_app::VERSION);
}

#[tokio::test]
async fn system_reports_machine_resources() {
    let (_tmp, router) = test_app();
    let (status, body) = get_json(router, "/api/v1/system").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["cpu_threads"].as_u64().unwrap() > 0);
    assert!(body["memory_total_bytes"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn unknown_api_route_returns_error_code() {
    let (_tmp, router) = test_app();
    let (status, body) = get_json(router, "/api/v1/does-not-exist").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "NOT_FOUND");
}

#[tokio::test]
async fn non_api_routes_serve_html() {
    let (_tmp, router) = test_app();
    let request = Request::get("/servers/abc").body(Body::empty()).unwrap();
    let response = router.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers()["content-type"].to_str().unwrap();
    assert!(content_type.starts_with("text/html"));
}

#[test]
fn openapi_lists_system_routes() {
    let spec = crate::openapi();
    assert!(spec.paths.paths.contains_key("/api/v1/health"));
    assert!(spec.paths.paths.contains_key("/api/v1/system"));
}
