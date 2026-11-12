//! `PATCH /servers/{id}` với `restart_at` (spec P2, mục 5.8).

use axum::http::{Method, StatusCode};
use serde_json::json;

#[tokio::test]
async fn restart_at_is_validated_and_sorted() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    let uri = format!("/api/v1/servers/{id}");

    let body = json!({ "restart_at": ["16:00", "04:00"] });
    let (status, server) = api.call(Method::PATCH, &uri, Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{server}");
    assert_eq!(server["restart_at"], json!(["04:00", "16:00"]));

    let (status, body) = api
        .call(Method::PATCH, &uri, Some(json!({ "restart_at": ["4:00"] })))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["params"]["field"], "restart_at");

    let five = json!({ "restart_at": ["00:00", "01:00", "02:00", "03:00", "05:00"] });
    let (status, _) = api.call(Method::PATCH, &uri, Some(five)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, server) = api
        .call(Method::PATCH, &uri, Some(json!({ "restart_at": [] })))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(server["restart_at"], json!([]));
}
