//! Forge với fixture lấy từ maven và `promotions_slim.json` thật (ngày 2026-11-13, đã cắt bớt).

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use mh_core::{ArgsOs, ErrorCode, Launch, LoaderVersion, McVersion};
use mh_net::{HostAllowlist, HttpClient, NetConfig};
use mh_providers::{Forge, Release};
use time::macros::datetime;
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture(name: &str) -> String {
    let path = format!("{}/fixtures/forge/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap()
}

const SHA: &str = "76c9f602faa3bfbfbb95a6041fbee23c7254f54749879ce4c821cbe1d662baa3";

async fn forge() -> (MockServer, tempfile::TempDir, Forge) {
    let server = MockServer::start().await;
    let routes = [
        ("/maven/maven-metadata.xml", fixture("maven-metadata.xml")),
        ("/promotions_slim.json", fixture("promotions_slim.json")),
    ];
    for (route, body) in routes {
        Mock::given(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
    }
    // Mọi file `.sha256` của installer trả cùng một hash; test chỉ cần đúng định dạng.
    Mock::given(wiremock::matchers::path_regex(r"^/maven/.+-installer\.jar\.sha256$"))
        .respond_with(ResponseTemplate::new(200).set_body_string(SHA))
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let mut config = NetConfig::new(tmp.path());
    config.retry_delays = vec![Duration::from_millis(1)];
    let http = Arc::new(HttpClient::new(config).unwrap());
    let forge = Forge::with_urls(
        http,
        format!("{}/maven", server.uri()),
        format!("{}/promotions_slim.json", server.uri()),
    )
    .with_hosts(HostAllowlist::new(&["127.0.0.1"]));
    (server, tmp, forge)
}

fn v(id: &str) -> McVersion {
    McVersion::parse(id).unwrap()
}

#[tokio::test]
async fn versions_start_at_1_12_2() {
    let (_s, _t, forge) = forge().await;
    let releases: Vec<Release> = [
        ("26.3", datetime!(2026-09-01 10:00 UTC)),
        ("1.21.4", datetime!(2024-12-03 10:12 UTC)),
        ("1.21.3", datetime!(2024-10-23 12:28 UTC)),
        ("1.12.2", datetime!(2017-09-18 08:39 UTC)),
        ("1.7.10", datetime!(2014-05-14 17:29 UTC)),
    ]
    .into_iter()
    .map(|(id, release_time)| Release { id: v(id), release_time })
    .collect();
    let got: Vec<String> = forge
        .versions(&releases)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.id.to_string())
        .collect();
    assert_eq!(got, ["26.3", "1.21.4", "1.12.2"]);
}

#[tokio::test]
async fn recommended_build_comes_first_then_newest() {
    let (_s, _t, forge) = forge().await;
    let got = forge.loader_versions(&v("1.20.1")).await.unwrap();
    let names: Vec<_> = got.iter().map(|r| r.version.as_str()).collect();
    assert_eq!(names, ["47.4.28", "47.4.5", "47.4.4", "47.4.3"]);
    assert!(got[0].stable);
    assert!(!got[1].stable);
    // Bản `26.3-66.0.0` đứng trước `66.0.2` trong file; MineHost tự sắp lại.
    let new = forge.loader_versions(&v("26.3")).await.unwrap();
    assert_eq!(new[0].version.as_str(), "66.0.2");
}

#[tokio::test]
async fn modern_forge_runs_from_args_file() {
    let (_s, _t, forge) = forge().await;
    let plan = forge.plan(&v("1.21.4"), None).await.unwrap();
    assert_eq!(plan.loader_version.as_str(), "54.1.22");
    let file = &plan.files[0];
    assert!(
        file.url
            .ends_with("/maven/1.21.4-54.1.22/forge-1.21.4-54.1.22-installer.jar"),
        "{}",
        file.url
    );
    assert_eq!(file.checksum.hex(), SHA);
    assert_eq!(plan.installer.unwrap().args, ["--installServer"]);
    let Launch::ArgsFile(args) = plan.launch else {
        panic!("Forge 1.21.4 chạy bằng file tham số");
    };
    assert_eq!(
        args.for_os(ArgsOs::Unix),
        "libraries/net/minecraftforge/forge/1.21.4-54.1.22/unix_args.txt"
    );
}

#[tokio::test]
async fn legacy_forge_runs_from_jar() {
    let (_s, _t, forge) = forge().await;
    let wanted = LoaderVersion::parse("36.2.42").unwrap();
    let plan = forge.plan(&v("1.16.5"), Some(&wanted)).await.unwrap();
    let Launch::Jar(jar) = plan.launch else {
        panic!("Forge 1.16.5 chạy bằng -jar");
    };
    assert_eq!(jar.as_str(), "forge-1.16.5-36.2.42.jar");
    let plan = forge.plan(&v("1.12.2"), None).await.unwrap();
    assert_eq!(plan.loader_version.as_str(), "14.23.5.2859");
}

#[tokio::test]
async fn old_or_unknown_versions_are_refused() {
    let (_s, _t, forge) = forge().await;
    let err = forge.plan(&v("1.7.10"), None).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::VersionNotFound, "{err}");
    let missing = LoaderVersion::parse("99.0.0").unwrap();
    let err = forge.plan(&v("1.21.4"), Some(&missing)).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::VersionNotFound, "{err}");
}

#[tokio::test]
async fn installer_must_come_from_listed_host() {
    let (s, t, _) = forge().await;
    let mut config = NetConfig::new(t.path().join("other"));
    config.retry_delays = vec![Duration::from_millis(1)];
    let http = Arc::new(HttpClient::new(config).unwrap());
    // Không gọi `with_hosts`: server giả không nằm trong `hosts::LOADERS`.
    let strict = Forge::with_urls(
        http,
        format!("{}/maven", s.uri()),
        format!("{}/promotions_slim.json", s.uri()),
    );
    let err = strict.plan(&v("1.21.4"), None).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::DownloadHostNotAllowed, "{err}");
}
