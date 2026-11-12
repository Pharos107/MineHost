//! Test Purpur với fixture lấy từ API v2 thật (ngày 2026-11-12), phục vụ qua `wiremock`.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use mh_core::{ErrorCode, McVersion};
use mh_net::{Checksum, HttpClient, NetConfig};
use mh_providers::{Purpur, Release};
use time::macros::datetime;
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture(name: &str) -> String {
    let path = format!("{}/fixtures/purpur/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap()
}

async fn purpur_with(routes: &[(&str, u16, String)]) -> (MockServer, tempfile::TempDir, Purpur) {
    let server = MockServer::start().await;
    for (route, status, body) in routes {
        Mock::given(path(format!("/v2/purpur{route}")))
            .respond_with(ResponseTemplate::new(*status).set_body_string(body.clone()))
            .mount(&server)
            .await;
    }
    let tmp = tempfile::tempdir().unwrap();
    let mut config = NetConfig::new(tmp.path());
    config.retry_delays = vec![Duration::from_millis(1)];
    let http = Arc::new(HttpClient::new(config).unwrap());
    let base = format!("{}/v2/purpur", server.uri());
    (server, tmp, Purpur::with_base_url(http, base))
}

fn v(id: &str) -> McVersion {
    McVersion::parse(id).unwrap()
}

fn release(id: &str, when: time::OffsetDateTime) -> Release {
    Release {
        id: v(id),
        release_time: when,
    }
}

#[tokio::test]
async fn versions_keep_only_mojang_releases_in_mojang_order() {
    let (_s, _t, purpur) = purpur_with(&[("", 200, fixture("project.json"))]).await;
    let releases = [
        release("1.21.4", datetime!(2024-12-03 10:12 UTC)),
        release("1.21.2", datetime!(2024-10-22 12:00 UTC)),
        release("1.16.5", datetime!(2021-01-14 16:05 UTC)),
        release("1.13.2", datetime!(2018-10-22 11:41 UTC)),
    ];
    let got: Vec<String> = purpur
        .versions(&releases)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.id.to_string())
        .collect();
    // Purpur bỏ qua 1.21.2 và không có bản trước 1.14.1.
    assert_eq!(got, ["1.21.4", "1.16.5"]);
}

#[tokio::test]
async fn latest_build_pins_build_number_and_md5() {
    let latest = fixture("latest-1.21.4.json");
    let (_s, _t, purpur) = purpur_with(&[("/1.21.4/latest", 200, latest)]).await;

    let build = purpur.latest_build(&v("1.21.4")).await.unwrap();
    assert_eq!(build.build, 2416);
    assert_eq!(
        build.jar.checksum,
        Checksum::md5("e3832b46efa4a2a56d31bc7ce26df6f0").unwrap()
    );
    assert!(build.jar.url.ends_with("/v2/purpur/1.21.4/2416/download"));
}

#[tokio::test]
async fn failed_build_is_not_installed() {
    let mut latest: serde_json::Value =
        serde_json::from_str(&fixture("latest-1.21.4.json")).unwrap();
    latest["result"] = "FAILURE".into();
    let (_s, _t, purpur) = purpur_with(&[("/1.21.4/latest", 200, latest.to_string())]).await;

    let err = purpur.latest_build(&v("1.21.4")).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::VersionNotFound);
}

#[tokio::test]
async fn bad_md5_is_rejected() {
    let mut latest: serde_json::Value =
        serde_json::from_str(&fixture("latest-1.21.4.json")).unwrap();
    latest["md5"] = "../../etc/passwd".into();
    let (_s, _t, purpur) = purpur_with(&[("/1.21.4/latest", 200, latest.to_string())]).await;

    assert!(purpur.latest_build(&v("1.21.4")).await.is_err());
}
