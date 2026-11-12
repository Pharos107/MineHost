//! Hồ sơ và skin Mojang qua wiremock. Không gọi mạng thật.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use base64::Engine;
use mh_core::{ErrorCode, PlayerName};
use mh_net::{HttpClient, NetConfig};
use mh_providers::{Mojang, ProviderError};
use wiremock::matchers::{any, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DASHED: &str = "069a79f4-44e9-4726-a5be-fca90e38aaf5";
const UNDASHED: &str = "069a79f444e94726a5befca90e38aaf5";
const SESSION: &str = "/session/minecraft/profile/069a79f444e94726a5befca90e38aaf5";
const TEXTURE: &str = "/texture/notch.png";

/// PNG 1×1. Chỉ cần đúng magic; bộ giải nén không được gọi.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
    0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xfe, 0x02, 0xfe, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

async fn players() -> (MockServer, tempfile::TempDir, Mojang) {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let mut config = NetConfig::new(tmp.path());
    config.retry_delays.clear();
    let http = Arc::new(HttpClient::new(config).unwrap());
    let origin = server.uri();
    let mojang = Mojang::with_player_urls(http, origin.clone(), origin.clone(), origin);
    (server, tmp, mojang)
}

fn textures_json(skin_url: &str) -> String {
    let payload = serde_json::json!({
        "textures": { "SKIN": { "url": skin_url } }
    });
    let value = base64::engine::general_purpose::STANDARD.encode(payload.to_string());
    serde_json::json!({
        "id": UNDASHED,
        "name": "Notch",
        "properties": [{ "name": "textures", "value": value }]
    })
    .to_string()
}

async fn calls(server: &MockServer) -> usize {
    server.received_requests().await.unwrap().len()
}

#[tokio::test]
async fn profile_returns_dashed_uuid_and_canonical_name() {
    let (server, _tmp, mojang) = players().await;
    Mock::given(path("/users/profiles/minecraft/notch"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(format!(r#"{{"id":"{UNDASHED}","name":"Notch"}}"#)),
        )
        .mount(&server)
        .await;

    let profile = mojang
        .profile_by_name(&PlayerName::parse("notch").unwrap())
        .await
        .unwrap();
    assert_eq!(profile.uuid, DASHED);
    assert_eq!(profile.name, "Notch");
}

#[tokio::test]
async fn missing_profile_is_player_not_found() {
    for status in [404u16, 204] {
        let (server, _tmp, mojang) = players().await;
        Mock::given(path("/users/profiles/minecraft/Ghost"))
            .respond_with(ResponseTemplate::new(status))
            .mount(&server)
            .await;
        let err = mojang
            .profile_by_name(&PlayerName::parse("Ghost").unwrap())
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::PlayerNotFound(_)), "{status}");
        assert_eq!(err.code(), ErrorCode::PlayerNotFound);
    }
}

#[tokio::test]
async fn skin_downloads_png_from_textures_property() {
    let (server, _tmp, mojang) = players().await;
    let body = textures_json(&format!("{}/texture/notch.png", server.uri()));
    Mock::given(path(SESSION))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;
    Mock::given(path(TEXTURE))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG))
        .mount(&server)
        .await;

    let png = mojang.skin(DASHED).await.unwrap();
    assert_eq!(png, PNG);
    assert_eq!(calls(&server).await, 2);
}

#[tokio::test]
async fn skin_on_another_host_is_refused_without_a_request() {
    let (good, _tmp, mojang) = players().await;
    let evil = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG))
        .mount(&evil)
        .await;
    let body = textures_json(&format!("{}/stolen.png", evil.uri()));
    Mock::given(path(SESSION))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&good)
        .await;

    let err = mojang.skin(UNDASHED).await.unwrap_err();
    assert!(matches!(err, ProviderError::Invalid { .. }));
    assert!(evil.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn fresh_skin_cache_skips_the_network() {
    let (server, tmp, mojang) = players().await;
    mount_skin(&server).await;

    assert_eq!(mojang.skin(DASHED).await.unwrap(), PNG);
    let after_first = calls(&server).await;
    assert_eq!(mojang.skin(DASHED).await.unwrap(), PNG);
    assert_eq!(calls(&server).await, after_first);
    let cached = tmp.path().join("skins").join(format!("{DASHED}.png"));
    assert_eq!(std::fs::read(cached).unwrap(), PNG);
}

#[tokio::test]
async fn expired_cache_is_kept_when_the_network_fails() {
    let (server, tmp, mojang) = players().await;
    Mock::given(path(SESSION))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(textures_json(&format!(
                "{}/texture/notch.png",
                server.uri()
            ))),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(path(TEXTURE))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    assert_eq!(mojang.skin(DASHED).await.unwrap(), PNG);
    let cached = tmp.path().join("skins").join(format!("{DASHED}.png"));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&cached)
        .unwrap();
    let old = SystemTime::now() - Duration::from_secs(25 * 60 * 60);
    file.set_modified(old).unwrap();
    drop(file);

    let before = calls(&server).await;
    assert_eq!(mojang.skin(DASHED).await.unwrap(), PNG);
    assert!(calls(&server).await > before);
}

#[tokio::test]
async fn profile_without_skin_is_a_distinct_error() {
    let (server, _tmp, mojang) = players().await;
    let payload = base64::engine::general_purpose::STANDARD.encode(r#"{"textures":{}}"#);
    let body = serde_json::json!({
        "id": UNDASHED,
        "name": "Notch",
        "properties": [{ "name": "textures", "value": payload }]
    })
    .to_string();
    Mock::given(path(SESSION))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let err = mojang.skin(DASHED).await.unwrap_err();
    assert!(matches!(err, ProviderError::NoSkin));
    assert_eq!(calls(&server).await, 1);
}

async fn mount_skin(server: &MockServer) {
    let body = textures_json(&format!("{}/texture/notch.png", server.uri()));
    Mock::given(path(SESSION))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
    Mock::given(path(TEXTURE))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG))
        .mount(server)
        .await;
}
