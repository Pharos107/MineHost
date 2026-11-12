//! Phiên, CSRF và khóa đăng nhập. `tests.rs` đã gần giới hạn 500 dòng.
#![allow(clippy::unwrap_used, clippy::panic)]

#[path = "auth_support.rs"]
pub(crate) mod support;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use support::{Api, HOST, NEXT, ORIGIN, PASSWORD, api, call, login};

#[tokio::test]
async fn no_password_keeps_existing_routes_open() {
    let api = api();
    let (status, body) = json_of(&api, Method::GET, "/api/v1/servers", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().is_some());

    let (status, body) = json_of(&api, Method::GET, "/api/v1/auth/session", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["password_set"], false);
    assert_eq!(body["logged_in"], false);
    assert!(body.get("csrf_token").is_none());

    let (status, body) = json_of(
        &api,
        Method::PUT,
        "/api/v1/auth/password",
        Some(json!({ "new": "1234567" })),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "PASSWORD_TOO_SHORT");
    assert!(!api.app.auth().password_set().unwrap());
}

#[tokio::test]
async fn password_requires_session_even_on_localhost() {
    let api = api();
    let (status, _) = json_of(
        &api,
        Method::PUT,
        "/api/v1/auth/password",
        Some(json!({ "new": PASSWORD })),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = json_of(&api, Method::GET, "/api/v1/servers", None, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "AUTH_REQUIRED");

    let (status, body) = json_of(&api, Method::GET, "/api/v1/auth/session", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["password_set"], true);
    assert_eq!(body["logged_in"], false);

    let bad = call(
        &api.router,
        Method::POST,
        "/api/v1/auth/login",
        Some(json!({ "password": NEXT })),
        None,
        None,
    )
    .await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);
    assert_eq!(bad.json()["code"], "AUTH_INVALID");

    let (cookie, csrf) = login(&api.router, PASSWORD).await;
    let (status, list) = json_of(
        &api,
        Method::GET,
        "/api/v1/servers",
        None,
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{list}");

    let (status, session) = json_of(
        &api,
        Method::GET,
        "/api/v1/auth/session",
        None,
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(session["logged_in"], true);
    assert_eq!(session["csrf_token"], csrf);
    assert_eq!(status, StatusCode::OK);

    let missing = call(
        &api.router,
        Method::POST,
        "/api/v1/servers",
        Some(json!({})),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(missing.status, StatusCode::FORBIDDEN);
    assert_eq!(missing.json()["code"], "FORBIDDEN");

    let wrong = call(
        &api.router,
        Method::POST,
        "/api/v1/servers",
        Some(json!({})),
        Some(&cookie),
        Some("not-the-token"),
    )
    .await;
    assert_eq!(wrong.status, StatusCode::FORBIDDEN);
    assert_eq!(wrong.json()["code"], "FORBIDDEN");
    let (status, _) = json_of(
        &api,
        Method::GET,
        "/api/v1/servers",
        None,
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let logged_out = call(
        &api.router,
        Method::POST,
        "/api/v1/auth/logout",
        None,
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(logged_out.status, StatusCode::NO_CONTENT);
    assert!(logged_out.set_cookie().is_some());
    let (status, body) = json_of(
        &api,
        Method::GET,
        "/api/v1/servers",
        None,
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "AUTH_REQUIRED");
}

#[tokio::test]
async fn sixth_login_is_rate_limited() {
    let api = api();
    api.app.auth().set_password(None, PASSWORD).unwrap();
    for _ in 0..5 {
        let reply = call(
            &api.router,
            Method::POST,
            "/api/v1/auth/login",
            Some(json!({ "password": "wrong-password" })),
            None,
            None,
        )
        .await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
        assert_eq!(reply.json()["code"], "AUTH_INVALID");
    }
    let blocked = call(
        &api.router,
        Method::POST,
        "/api/v1/auth/login",
        Some(json!({ "password": PASSWORD })),
        None,
        None,
    )
    .await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(blocked.json()["code"], "AUTH_RATE_LIMITED");
    let wait = blocked.json()["params"]["retry_after_s"].as_u64().unwrap();
    assert!(wait > 0, "{wait}");
    let header = blocked
        .headers
        .get(header::RETRY_AFTER)
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(header, wait.to_string());
}

#[tokio::test]
async fn changing_password_drops_existing_sessions() {
    let api = api();
    api.app.auth().set_password(None, PASSWORD).unwrap();
    let (cookie, csrf) = login(&api.router, PASSWORD).await;
    let changed = call(
        &api.router,
        Method::PUT,
        "/api/v1/auth/password",
        Some(json!({ "current": PASSWORD, "new": NEXT })),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(changed.status, StatusCode::NO_CONTENT, "{}", changed.json());
    let (status, _) = json_of(
        &api,
        Method::GET,
        "/api/v1/servers",
        None,
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (cookie, csrf) = login(&api.router, NEXT).await;

    let rejected = call(
        &api.router,
        Method::DELETE,
        "/api/v1/auth/password",
        Some(json!({ "current": PASSWORD })),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(rejected.status, StatusCode::UNAUTHORIZED);
    assert!(api.app.auth().password_set().unwrap());

    let cleared = call(
        &api.router,
        Method::DELETE,
        "/api/v1/auth/password",
        Some(json!({ "current": NEXT })),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(cleared.status, StatusCode::NO_CONTENT, "{}", cleared.json());
    assert!(!api.app.auth().password_set().unwrap());
    assert!(!api.app.auth().lan().unwrap());
    let (status, _) = json_of(&api, Method::GET, "/api/v1/servers", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn static_files_and_websocket_need_no_session_cookie_only_when_open() {
    let api = api();
    api.app.auth().set_password(None, PASSWORD).unwrap();

    let page = call(&api.router, Method::GET, "/", None, None, None).await;
    assert_eq!(page.status, StatusCode::OK);
    let content_type = page
        .headers
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.starts_with("text/html"), "{content_type}");
    assert!(!page.json()["code"].as_str().unwrap_or("").contains("AUTH"));

    let builder = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/ws")
        .header(header::HOST, HOST)
        .header(header::ORIGIN, ORIGIN)
        .header(header::UPGRADE, "websocket")
        .header(header::CONNECTION, "Upgrade")
        .header(header::SEC_WEBSOCKET_VERSION, "13")
        .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==");
    let mut request = builder.body(Body::empty()).unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 9))));
    let response = api.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], "AUTH_REQUIRED");
}

async fn json_of(
    api: &Api,
    method: Method,
    uri: &str,
    body: Option<Value>,
    cookie: Option<&str>,
    csrf: Option<&str>,
) -> (StatusCode, Value) {
    let reply = call(&api.router, method, uri, body, cookie, csrf).await;
    (reply.status, reply.json())
}
