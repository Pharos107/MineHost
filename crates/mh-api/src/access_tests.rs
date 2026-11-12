//! `/settings/access` và request đi qua listener LAN (spec P2, mục 5.8, 5.9).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::json;

use crate::LanState;
use crate::auth_tests::support::{PASSWORD, api, call, call_at, login};

const LAN_HOST: &str = "10.1.2.3:7878";

/// Đóng vai binary: mỗi lần API báo cờ `lan` đổi thì mở hoặc đóng "listener" theo cờ đó.
fn fake_binary(app: Arc<mh_app::App>, lan: Arc<LanState>) {
    tokio::spawn(async move {
        loop {
            lan.woken().await;
            let open = if app.auth().lan().unwrap() {
                vec![LAN_HOST.parse().unwrap()]
            } else {
                Vec::new()
            };
            lan.set_open(open);
        }
    });
}

#[tokio::test]
async fn lan_needs_password_and_lists_open_urls() {
    let api = api();
    let lan = LanState::new();
    fake_binary(Arc::clone(&api.app), Arc::clone(&lan));
    let router = crate::router_with_lan(Arc::clone(&api.app), 7878, Arc::clone(&lan));

    let body = Some(json!({ "lan": true }));
    let reply = call(
        &router,
        Method::PUT,
        "/api/v1/settings/access",
        body,
        None,
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.json()["code"], "LAN_NEEDS_PASSWORD");

    api.app.auth().set_password(None, PASSWORD).unwrap();
    let (cookie, csrf) = login(&router, PASSWORD).await;
    let body = Some(json!({ "lan": true }));
    let reply = call(
        &router,
        Method::PUT,
        "/api/v1/settings/access",
        body,
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json());
    assert_eq!(
        reply.json(),
        json!({ "lan": true, "lan_urls": ["http://10.1.2.3:7878"] })
    );

    let body = Some(json!({ "lan": false }));
    let reply = call(
        &router,
        Method::PUT,
        "/api/v1/settings/access",
        body,
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(reply.json(), json!({ "lan": false, "lan_urls": [] }));
    assert!(lan.open_addrs().is_empty());
}

#[tokio::test]
async fn lan_host_is_accepted_only_while_listener_is_open() {
    let api = api();
    let lan = LanState::new();
    let router = crate::router_with_lan(Arc::clone(&api.app), 7878, Arc::clone(&lan));
    api.app.auth().set_password(None, PASSWORD).unwrap();
    api.app.auth().set_lan(true).unwrap();

    let reply = call_at(
        &router,
        LAN_HOST,
        Method::GET,
        "/api/v1/auth/session",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.json()["params"]["reason"], "host");

    lan.set_open(vec![LAN_HOST.parse().unwrap()]);
    let body = Some(json!({ "password": PASSWORD }));
    let reply = call_at(
        &router,
        LAN_HOST,
        Method::POST,
        "/api/v1/auth/login",
        body,
        None,
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json());

    // Listener khác cổng hoặc khác IP vẫn bị chặn.
    let reply = call_at(
        &router,
        "10.1.2.3:7879",
        Method::GET,
        "/api/v1/auth/session",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn lan_requests_are_refused_once_password_is_reset() {
    let api = api();
    let lan = LanState::new();
    let router = crate::router_with_lan(Arc::clone(&api.app), 7878, Arc::clone(&lan));
    let lan_router = crate::lan_listener(router.clone(), Arc::clone(&lan));
    api.app.auth().set_password(None, PASSWORD).unwrap();
    api.app.auth().set_lan(true).unwrap();
    lan.set_open(vec![LAN_HOST.parse().unwrap()]);

    // `reset-password` xóa mật khẩu trong lúc listener LAN chưa kịp đóng.
    api.app.auth().reset().unwrap();
    for uri in ["/api/v1/auth/session", "/api/v1/servers"] {
        let reply = call_at(&lan_router, LAN_HOST, Method::GET, uri, None, None, None).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{uri}");
        assert_eq!(reply.json()["params"]["reason"], "lan", "{uri}");
    }
    // Client trong LAN đổi `Host` thành localhost cũng không giả được là máy này.
    let reply = call_at(
        &lan_router,
        "127.0.0.1:7878",
        Method::GET,
        "/api/v1/servers",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.json()["params"]["reason"], "host");
    // Trên máy này thì như P1: chưa có mật khẩu là vào được.
    let reply = call(&router, Method::GET, "/api/v1/servers", None, None, None).await;
    assert_eq!(reply.status, StatusCode::OK);
}
