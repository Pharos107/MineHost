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

#[tokio::test]
async fn java_major_is_validated_and_saved() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    let uri = format!("/api/v1/servers/{id}");

    let (status, server) = api
        .call(Method::PATCH, &uri, Some(json!({ "java_major": 17 })))
        .await;
    assert_eq!(status, StatusCode::OK, "{server}");
    assert_eq!(server["java_major"], 17);
    // Bản Mojang yêu cầu không đổi theo lựa chọn của người dùng, để UI còn cảnh báo được.
    assert_eq!(server["java_recommended"], 21);

    let (status, system) = api.get("/api/v1/system").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(system["java_majors"], json!([8, 11, 16, 17, 21, 25]));

    let (status, body) = api
        .call(Method::PATCH, &uri, Some(json!({ "java_major": 19 })))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "INVALID_INPUT");
    assert_eq!(body["params"]["field"], "java_major");
}
