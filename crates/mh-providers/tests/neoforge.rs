//! NeoForge với fixture lấy từ maven thật (ngày 2026-11-13, đã cắt bớt), qua `wiremock`.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use mh_core::{ArgsOs, ErrorCode, Launch, McVersion};
use mh_net::{HostAllowlist, HttpClient, NetConfig};
use mh_providers::{NeoForge, Release};
use time::macros::datetime;
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture(name: &str) -> String {
    let path = format!("{}/fixtures/neoforge/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap()
}

async fn neoforge() -> (MockServer, tempfile::TempDir, NeoForge) {
    let server = MockServer::start().await;
    Mock::given(path("/maven/maven-metadata.xml"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("maven-metadata.xml")))
        .mount(&server)
        .await;
    Mock::given(path("/maven/21.4.161/neoforge-21.4.161-installer.jar.sha256"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(fixture("neoforge-21.4.161-installer.jar.sha256")),
        )
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let mut config = NetConfig::new(tmp.path());
    config.retry_delays = vec![Duration::from_millis(1)];
    let http = Arc::new(HttpClient::new(config).unwrap());
    let neo = NeoForge::with_base_url(http, format!("{}/maven", server.uri()))
        .with_hosts(HostAllowlist::new(&["127.0.0.1"]));
    (server, tmp, neo)
}

fn v(id: &str) -> McVersion {
    McVersion::parse(id).unwrap()
}

#[tokio::test]
async fn versions_follow_mapping() {
    let (_s, _t, neo) = neoforge().await;
    let releases: Vec<Release> = [
        ("26.3", datetime!(2026-09-01 10:00 UTC)),
        ("26.1.2", datetime!(2026-05-01 10:00 UTC)),
        ("1.21.4", datetime!(2024-12-03 10:12 UTC)),
        ("1.21", datetime!(2024-06-13 08:24 UTC)),
        ("1.20.1", datetime!(2023-06-12 13:25 UTC)),
    ]
    .into_iter()
    .map(|(id, release_time)| Release { id: v(id), release_time })
    .collect();
    let got: Vec<String> = neo
        .versions(&releases)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.id.to_string())
        .collect();
    assert_eq!(got, ["26.3", "26.1.2", "1.21.4", "1.21"]);
}

#[tokio::test]
async fn betas_are_hidden_when_a_stable_build_exists() {
    let (_s, _t, neo) = neoforge().await;
    let got = neo.loader_versions(&v("1.21.4")).await.unwrap();
    let names: Vec<_> = got.iter().map(|r| r.version.as_str()).collect();
    assert_eq!(names, ["21.4.161", "21.4.158"]);
    // 26.1 chỉ có bản beta: vẫn cài được, bản mới nhất đứng đầu.
    let beta_only = neo.loader_versions(&v("26.1")).await.unwrap();
    assert_eq!(beta_only[0].version.as_str(), "26.1.0.19-beta");
    assert!(beta_only.iter().all(|r| !r.stable));
}

#[tokio::test]
async fn plan_runs_from_args_file() {
    let (_s, _t, neo) = neoforge().await;
    let plan = neo.plan(&v("1.21.4"), None).await.unwrap();
    assert_eq!(plan.loader_version.as_str(), "21.4.161");
    let file = &plan.files[0];
    assert!(
        file.url
            .ends_with("/maven/21.4.161/neoforge-21.4.161-installer.jar"),
        "{}",
        file.url
    );
    assert_eq!(file.checksum.hex().len(), 64);
    assert_eq!(file.dest, ".minehost-install/neoforge-installer-21.4.161.jar");
    assert_eq!(plan.installer.unwrap().args, ["--installServer"]);
    let Launch::ArgsFile(args) = plan.launch else {
        panic!("NeoForge chạy bằng file tham số");
    };
    assert_eq!(
        args.for_os(ArgsOs::Windows),
        "libraries/net/neoforged/neoforge/21.4.161/win_args.txt"
    );
}

#[tokio::test]
async fn neoforge_1_20_1_is_not_supported() {
    let (_s, _t, neo) = neoforge().await;
    let err = neo.plan(&v("1.20.1"), None).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::VersionNotFound, "{err}");
}
