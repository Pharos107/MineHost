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
    // `forge`, `fabric` là loại đã biết nhưng chưa cài được (spec P3, PR-09); `folia` là loại lạ.
    for loader in ["forge", "fabric", "folia"] {
        let (status, body) = api.get(&format!("/api/v1/catalog/{loader}/versions")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{loader}");
        assert_eq!(body["code"], "INVALID_INPUT");
        assert_eq!(body["params"]["field"], "loader");
    }
}
