//! Route người chơi (spec P2, mục 5.8 và 6.7; kế hoạch PR-17).

use axum::http::StatusCode;
use mh_core::ServerId;
use serde_json::json;

use super::TestApi;

/// Đặt một khóa trong `server.properties`, giữ các khóa khác.
fn set_property(api: &TestApi, id: &str, key: &str, value: &str) {
    let id = ServerId::parse(id).unwrap();
    let mut props = api.app.read_properties(id).unwrap();
    props.retain(|(k, _)| k != key);
    props.push((key.to_owned(), value.to_owned()));
    api.app.write_properties(id, &props).unwrap();
}

#[tokio::test]
async fn stopped_server_edits_files_and_refuses_live_actions() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    set_property(&api, &id, "online-mode", "false");
    let actions = |name: &str| format!("/api/v1/servers/{id}/players/{name}/actions");

    let (status, body) = api
        .post(
            &actions("Steve"),
            Some(json!({ "action": "whitelist_add" })),
        )
        .await;
    assert_eq!(
        (status, body["done"].as_str()),
        (StatusCode::OK, Some("saved"))
    );
    let (status, body) = api
        .post(
            &actions("Alex"),
            Some(json!({ "action": "ban", "reason": "  phá nhà " })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, list) = api.get(&format!("/api/v1/servers/{id}/players")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (
            list["running"].as_bool(),
            list["online_mode"].as_bool(),
            list["max_players"].as_u64()
        ),
        (Some(false), Some(false), Some(20))
    );
    let players = list["players"].as_array().unwrap();
    let steve = players.iter().find(|p| p["name"] == "Steve").unwrap();
    assert_eq!(steve["whitelisted"], true);
    // UUID offline của "Steve", như Minecraft tính.
    assert_eq!(steve["uuid"], "5627dd98-e6be-3c21-b8a8-e92344183641");
    let alex = players.iter().find(|p| p["name"] == "Alex").unwrap();
    assert_eq!(
        (alex["banned"].as_bool(), alex["ban_reason"].as_str()),
        (Some(true), Some("phá nhà"))
    );

    for action in ["kick", "freeze", "teleport_to_spawn"] {
        let (status, body) = api
            .post(&actions("Steve"), Some(json!({ "action": action })))
            .await;
        assert_eq!(status, StatusCode::CONFLICT, "{action}");
        assert_eq!(body["code"], "SERVER_NOT_RUNNING", "{action}");
    }
}

#[tokio::test]
async fn online_mode_needs_account_lookup() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    let url = format!("/api/v1/servers/{id}/players/Notch/actions");
    // Bản test không có nguồn tra Mojang: giống mất mạng, không được ghi UUID đoán bừa.
    let (status, body) = api.post(&url, Some(json!({ "action": "op" }))).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["code"], "NETWORK_UNAVAILABLE");
}

#[tokio::test]
async fn bad_names_and_bodies_are_rejected() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    let base = format!("/api/v1/servers/{id}/players");
    for (name, body) in [
        ("ab", json!({ "action": "op" })),
        ("bad name", json!({ "action": "op" })),
        ("Steve", json!({ "action": "fly" })),
        (
            "Steve",
            json!({ "action": "teleport_to_player", "target": "x" }),
        ),
        ("Steve", json!({ "action": "message", "text": "  " })),
        ("Steve", json!({ "action": "message", "text": "a\nb" })),
        (
            "Steve",
            json!({ "action": "kick", "reason": "x".repeat(201) }),
        ),
    ] {
        let url = format!("{base}/{}/actions", name.replace(' ', "%20"));
        let (status, _) = api.post(&url, Some(body.clone())).await;
        assert!(status.is_client_error(), "{name} {body} → {status}");
        assert_ne!(status, StatusCode::NOT_FOUND, "{name} {body}");
    }
    let (status, body) = api
        .get("/api/v1/servers/01JAAAAAAAAAAAAAAAAAAAAAAA/players")
        .await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("NOT_FOUND"))
    );
}

#[tokio::test]
async fn skin_without_source_is_bad_gateway() {
    let api = super::test_api();
    let (status, body) = api
        .get("/api/v1/players/069a79f4-44e9-4726-a5be-fca90e38aaf5/skin")
        .await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::BAD_GATEWAY, Some("NETWORK_UNAVAILABLE"))
    );
}
