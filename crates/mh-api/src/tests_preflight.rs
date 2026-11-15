//! Test `oneshot` cho route `/servers/{id}/preflight` và `actions/start` nhận `ignore_preflight`.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use serde_json::{Value, json};

use super::super::{HOST, ORIGIN, send};
use super::catalog::store_zip;
use super::support::{content_api, create_with_loader, upload_jar};

#[tokio::test]
async fn preflight_endpoint_and_start_action_with_or_without_body() {
    let api = content_api(None);
    let id = create_with_loader(&api, "Fabric", "fabric", 25565).await;

    let (status, initial) = api.get(&format!("/api/v1/servers/{id}/preflight")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        initial,
        json!({
            "missing_deps": [],
            "version_mismatches": [],
            "wrong_loader": [],
            "pending": [],
            "client_only": [],
        })
    );
    let (_, server_dto) = api.get(&format!("/api/v1/servers/{id}")).await;
    assert_eq!(server_dto["preflight_blocking"], false);

    // Upload mod thiếu dependency + chỉ client.
    let jar = store_zip(
        "fabric.mod.json",
        br#"{"id":"broken","name":"Broken Mod","version":"1.0.0","environment":"client","depends":{"absent-lib":">=1.2.0"}}"#,
    );
    let (up_status, _) = upload_jar(
        &api.router,
        &format!("/api/v1/servers/{id}/content/upload"),
        "broken.jar",
        &jar,
    )
    .await;
    assert_eq!(up_status, StatusCode::OK);

    let (_, report) = api.get(&format!("/api/v1/servers/{id}/preflight")).await;
    assert_eq!(
        report["missing_deps"],
        json!([{ "id": "absent-lib", "version_req": ">=1.2.0", "required_by": "Broken Mod" }])
    );
    assert_eq!(
        report["client_only"],
        json!([{ "file_name": "broken.jar", "name": "Broken Mod" }])
    );

    let (_, server_dto) = api.get(&format!("/api/v1/servers/{id}")).await;
    assert_eq!(server_dto["preflight_blocking"], true);

    // Gọi start KHÔNG có body -> giữ nguyên hành vi cũ, bị chặn bởi preflight.
    let start_uri = format!("/api/v1/servers/{id}/actions/start");
    let (status, err) = api.post(&start_uri, None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["code"], "DEPENDENCY_MISSING");
    assert_eq!(err["params"]["missing"][0]["id"], "absent-lib");

    // Body `{}` hay `{"ignore_preflight": false}` vẫn bị chặn.
    let (status, err) = api.post(&start_uri, Some(json!({}))).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["code"], "DEPENDENCY_MISSING");

    // Body `{"ignore_preflight": true}` bỏ qua preflight (tới bước EULA).
    let (status, err) = api
        .post(&start_uri, Some(json!({ "ignore_preflight": true })))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["code"], "EULA_NOT_ACCEPTED");
}

#[tokio::test]
async fn start_action_rejects_malformed_or_non_json_body_and_openapi_lists_preflight() {
    let api = content_api(None);
    let id = create_with_loader(&api, "Paper", "paper", 25565).await;
    let start_uri = format!("/api/v1/servers/{id}/actions/start");

    let bad_json = Request::builder()
        .method(Method::POST)
        .uri(&start_uri)
        .header(header::HOST, HOST)
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{bad"))
        .unwrap();
    let (status, body) = send(api.router.clone(), bad_json).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "INVALID_INPUT");

    let wrong_ct = Request::builder()
        .method(Method::POST)
        .uri(&start_uri)
        .header(header::HOST, HOST)
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from("hello"))
        .unwrap();
    let (status, body) = send(api.router.clone(), wrong_ct).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(body["code"], "INVALID_INPUT");

    let (status, _) = api
        .get("/api/v1/servers/01J00000000000000000000000/preflight")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let spec = crate::openapi();
    assert!(
        spec.paths
            .paths
            .contains_key("/api/v1/servers/{id}/preflight")
    );
    let _ = Value::Null;
}
