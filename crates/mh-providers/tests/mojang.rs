//! Test Mojang với fixture rút gọn từ API thật (ngày 2026-10-23), phục vụ qua `wiremock`.
//!
//! Fixture JSON của từng phiên bản đã bỏ các khóa không dùng, nên `sha1` trong manifest
//! được tính lại theo file rút gọn.

// Hàm trợ giúp ngoài `#[test]` không được clippy coi là code test.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use mh_core::{ErrorCode, McVersion};
use mh_net::{HttpClient, NetConfig};
use mh_providers::Mojang;
use sha1::{Digest, Sha1};
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

const REAL_HOST: &str = "https://piston-meta.mojang.com";

fn fixture(name: &str) -> String {
    let path = format!("{}/fixtures/mojang/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap()
}

fn http(tmp: &tempfile::TempDir) -> Arc<HttpClient> {
    let mut config = NetConfig::new(tmp.path());
    config.retry_delays = vec![Duration::from_millis(1)];
    Arc::new(HttpClient::new(config).unwrap())
}

async fn serve(server: &MockServer, route: &str, body: String) {
    Mock::given(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
}

/// Phục vụ manifest (đã đổi host sang server giả) và JSON của các phiên bản có fixture.
async fn mojang_with(manifest: String) -> (MockServer, tempfile::TempDir, Mojang) {
    let server = MockServer::start().await;
    serve(
        &server,
        "/manifest.json",
        manifest.replace(REAL_HOST, &server.uri()),
    )
    .await;
    let parsed: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    for id in ["26.3", "1.21.4", "1.16.5"] {
        let entry = parsed["versions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == id)
            .unwrap();
        let url = entry["url"].as_str().unwrap();
        serve(
            &server,
            url.trim_start_matches(REAL_HOST),
            fixture(&format!("{id}.json")),
        )
        .await;
    }
    let tmp = tempfile::tempdir().unwrap();
    let url = format!("{}/manifest.json", server.uri());
    let mojang = Mojang::with_manifest_url(http(&tmp), url);
    (server, tmp, mojang)
}

fn v(id: &str) -> McVersion {
    McVersion::parse(id).unwrap()
}

#[tokio::test]
async fn releases_are_newest_first_without_snapshots() {
    let (_server, _tmp, mojang) = mojang_with(fixture("version_manifest_v2.json")).await;
    let ids: Vec<String> = mojang
        .releases()
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.id.to_string())
        .collect();
    assert_eq!(ids, ["26.3", "1.21.4", "1.16.5"]);
}

#[tokio::test]
async fn java_major_per_version() {
    let (_server, _tmp, mojang) = mojang_with(fixture("version_manifest_v2.json")).await;
    for (id, java) in [("26.3", 25), ("1.21.4", 21), ("1.16.5", 8)] {
        let detail = mojang.version_detail(&v(id)).await.unwrap();
        assert_eq!(detail.java_major, java, "{id}");
    }
}

#[tokio::test]
async fn server_jar_has_url_sha1_and_size() {
    let (_server, _tmp, mojang) = mojang_with(fixture("version_manifest_v2.json")).await;
    let jar = mojang
        .version_detail(&v("1.21.4"))
        .await
        .unwrap()
        .server
        .unwrap();
    assert_eq!(
        jar.checksum.hex(),
        "4707d00eb834b446575d89a61a11b5d548d8c001"
    );
    assert_eq!(jar.size, 56_880_250);
    assert!(jar.url.ends_with("/server.jar"));
}

#[tokio::test]
async fn missing_java_version_means_java_8() {
    let detail = r#"{"id":"1.2.5","type":"release","downloads":{}}"#;
    let sha1: String = Sha1::digest(detail)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let manifest = format!(
        r#"{{"versions":[
            {{"id":"26.3","type":"release","url":"{REAL_HOST}/x/26.3.json","sha1":"{sha1}","releaseTime":"2026-09-15T11:23:02+00:00"}},
            {{"id":"1.21.4","type":"release","url":"{REAL_HOST}/x/1.21.4.json","sha1":"{sha1}","releaseTime":"2024-12-03T10:12:57+00:00"}},
            {{"id":"1.16.5","type":"release","url":"{REAL_HOST}/x/1.16.5.json","sha1":"{sha1}","releaseTime":"2021-01-14T16:05:32+00:00"}},
            {{"id":"1.2.5","type":"release","url":"{REAL_HOST}/old/1.2.5.json","sha1":"{sha1}","releaseTime":"2012-03-29T22:00:00+00:00"}}
        ]}}"#
    );
    let (server, _tmp, mojang) = mojang_with(manifest).await;
    serve(&server, "/old/1.2.5.json", detail.to_owned()).await;

    let got = mojang.version_detail(&v("1.2.5")).await.unwrap();
    assert_eq!(got.java_major, 8);
    assert_eq!(got.server, None);
}

#[tokio::test]
async fn unknown_version_is_version_not_found() {
    let (_server, _tmp, mojang) = mojang_with(fixture("version_manifest_v2.json")).await;
    let err = mojang.version_detail(&v("9.9.9")).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::VersionNotFound);
}

#[tokio::test]
async fn detail_with_wrong_sha1_is_rejected() {
    let (server, _tmp, mojang) = mojang_with(fixture("version_manifest_v2.json")).await;
    // Snapshot này trỏ tới URL thật, nhưng server giả trả JSON của bản khác: sai sha1.
    let route = "/v1/packages/2ee2d20f6e6a37aae39238fff3286ab879a99ce6/26.4-snapshot-5.json";
    serve(&server, route, fixture("26.3.json")).await;

    let err = mojang
        .version_detail(&v("26.4-snapshot-5"))
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::ChecksumMismatch);
}

/// Gọi API thật: `cargo test -p mh-providers -- --ignored`.
#[tokio::test]
#[ignore = "gọi API thật của Mojang"]
async fn real_api_lists_1_21_4_with_java_21() {
    let tmp = tempfile::tempdir().unwrap();
    let mojang = Mojang::new(http(&tmp));
    let releases = mojang.releases().await.unwrap();
    assert!(releases.iter().any(|r| r.id.as_str() == "1.21.4"));
    let detail = mojang.version_detail(&v("1.21.4")).await.unwrap();
    assert_eq!(detail.java_major, 21);
}
