//! `/catalog/{loader}/versions` cho Paper, Vanilla, Purpur (spec P2, mục 5.8).

use axum::http::StatusCode;

#[tokio::test]
async fn catalog_accepts_every_supported_loader() {
    let api = super::test_api();
    for loader in ["paper", "vanilla", "purpur"] {
        let (status, body) = api.get(&format!("/api/v1/catalog/{loader}/versions")).await;
        assert_eq!(status, StatusCode::OK, "{loader}: {body}");
        assert_eq!(body[0]["id"], "1.21.4", "{loader}");
    }
    for loader in ["folia", "Paper"] {
        let (status, body) = api.get(&format!("/api/v1/catalog/{loader}/versions")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{loader}");
        assert_eq!(body["code"], "INVALID_INPUT");
        assert_eq!(body["params"]["field"], "loader");
    }
}

#[tokio::test]
async fn loader_versions_only_for_mod_loaders() {
    let api = super::test_api();
    let (status, body) = api
        .get("/api/v1/catalog/fabric/1.21.4/loader-versions")
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body[0]["version"], "0.16.9");
    assert_eq!(body[0]["stable"], true);
    assert_eq!(body[1]["stable"], false);
    for path in [
        "/api/v1/catalog/paper/1.21.4/loader-versions",
        "/api/v1/catalog/fabric/..%2Fx/loader-versions",
    ] {
        let (status, body) = api.get(path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}: {body}");
    }
}
