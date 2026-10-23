//! Test Paper với fixture rút gọn từ Fill API v3 thật (ngày 2026-10-23), phục vụ qua `wiremock`.

// Hàm trợ giúp ngoài `#[test]` không được clippy coi là code test.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use mh_core::{ErrorCode, McVersion};
use mh_net::{HttpClient, NetConfig};
use mh_providers::{Paper, Release};
use time::macros::datetime;
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture(name: &str) -> String {
    let path = format!("{}/fixtures/paper/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap()
}

async fn paper_with(routes: &[(&str, u16, String)]) -> (MockServer, tempfile::TempDir, Paper) {
    let server = MockServer::start().await;
    for (route, status, body) in routes {
        Mock::given(path(format!("/v3/projects/paper{route}")))
            .respond_with(ResponseTemplate::new(*status).set_body_string(body.clone()))
            .mount(&server)
            .await;
    }
    let tmp = tempfile::tempdir().unwrap();
    let mut config = NetConfig::new(tmp.path());
    config.retry_delays = vec![Duration::from_millis(1)];
    let http = Arc::new(HttpClient::new(config).unwrap());
    let base = format!("{}/v3/projects/paper", server.uri());
    (server, tmp, Paper::with_base_url(http, base))
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
    let (_s, _t, paper) = paper_with(&[("", 200, fixture("project.json"))]).await;
    let releases = [
        release("26.3", datetime!(2026-09-15 11:23 UTC)),
        release("1.21.11", datetime!(2025-12-09 12:00 UTC)),
        release("1.21.4", datetime!(2024-12-03 10:12 UTC)),
        release("1.21.2", datetime!(2024-10-22 12:00 UTC)),
        release("1.16.5", datetime!(2021-01-14 16:05 UTC)),
        release("1.2.5", datetime!(2012-03-29 22:00 UTC)),
    ];

    let got: Vec<String> = paper
        .versions(&releases)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.id.to_string())
        .collect();
    // 1.21.2 và 1.2.5 không có trên Paper; `-rc`, `-pre` của Paper không phải release của Mojang.
    assert_eq!(got, ["26.3", "1.21.11", "1.21.4", "1.16.5"]);
}

#[tokio::test]
async fn latest_stable_build_reads_server_download() {
    let builds = fixture("builds-1.21.4.json");
    let (_s, _t, paper) = paper_with(&[("/versions/1.21.4/builds", 200, builds)]).await;

    let build = paper.latest_stable_build(&v("1.21.4")).await.unwrap();
    assert_eq!(build.build, 232);
    assert_eq!(
        build.jar.checksum.hex(),
        "5ee4f542f628a14c644410b08c94ea42e772ef4d29fe92973636b6813d4eaffc"
    );
    assert_eq!(build.jar.size, 51_437_498);
    assert!(build.jar.url.ends_with("/paper-1.21.4-232.jar"));
}

#[tokio::test]
async fn newer_builds_that_are_not_stable_are_skipped() {
    let mut builds: Vec<serde_json::Value> =
        serde_json::from_str(&fixture("builds-1.21.4.json")).unwrap();
    let mut alpha = builds[0].clone();
    alpha["id"] = 999.into();
    alpha["channel"] = "ALPHA".into();
    let mut beta = builds[0].clone();
    beta["id"] = 998.into();
    beta["channel"] = "BETA".into();
    builds.insert(0, alpha);
    builds.insert(0, beta);
    let body = serde_json::to_string(&builds).unwrap();
    let (_s, _t, paper) = paper_with(&[("/versions/1.21.4/builds", 200, body)]).await;

    let build = paper.latest_stable_build(&v("1.21.4")).await.unwrap();
    assert_eq!(build.build, 232);
}

#[tokio::test]
async fn no_stable_build_means_version_not_found() {
    let body = r#"[{"id":5,"channel":"ALPHA","downloads":{}}]"#.to_owned();
    let (_s, _t, paper) = paper_with(&[("/versions/26.4/builds", 200, body)]).await;
    let err = paper.latest_stable_build(&v("26.4")).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::VersionNotFound);
}

#[tokio::test]
async fn unknown_version_404_means_version_not_found() {
    let (_s, _t, paper) = paper_with(&[("/versions/9.9/builds", 404, "{}".to_owned())]).await;
    let err = paper.latest_stable_build(&v("9.9")).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::VersionNotFound);
}

#[tokio::test]
async fn bad_checksum_format_is_invalid_data() {
    let body = fixture("builds-1.21.4.json").replace("5ee4f542", "not-hex!");
    let (_s, _t, paper) = paper_with(&[("/versions/1.21.4/builds", 200, body)]).await;
    let err = paper.latest_stable_build(&v("1.21.4")).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::NetworkUnavailable);
}

#[test]
fn recipe_runs_server_jar_without_gui() {
    let recipe = Paper::recipe();
    assert_eq!(recipe.jar.as_str(), "server.jar");
    assert_eq!(recipe.args, ["--nogui"]);
}

/// Gọi API thật: `cargo test -p mh-providers -- --ignored`.
#[tokio::test]
#[ignore = "gọi API thật của PaperMC"]
async fn real_api_has_stable_1_21_4() {
    let tmp = tempfile::tempdir().unwrap();
    let http = Arc::new(HttpClient::new(NetConfig::new(tmp.path())).unwrap());
    let build = Paper::new(http)
        .latest_stable_build(&v("1.21.4"))
        .await
        .unwrap();
    assert!(build.build >= 232);
}
