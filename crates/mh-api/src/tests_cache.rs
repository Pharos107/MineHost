//! Bộ nhớ đệm tải về và trạng thái giới hạn tài nguyên (spec P3, mục 6.5; kế hoạch PR-35).

use axum::http::{Method, StatusCode};

use super::support::content_api;

#[tokio::test]
async fn cache_size_clear_and_resource_limits() {
    let api = content_api(None);
    let (status, cache) = api.get("/api/v1/settings/download-cache").await;
    assert_eq!(status, StatusCode::OK, "{cache}");
    assert!(cache["size_bytes"].is_u64(), "{cache}");

    let (status, cleared) = api
        .call(Method::DELETE, "/api/v1/settings/download-cache", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert!(cleared["freed_bytes"].is_u64(), "{cleared}");

    let (status, system) = api.get("/api/v1/system").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        system["resource_limits"]["enforced"].is_boolean(),
        "{system}"
    );
}
