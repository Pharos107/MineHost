//! Fabric và Quilt với fixture lấy từ meta thật (ngày 2026-11-13, đã cắt bớt), qua `wiremock`.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use mh_core::{ErrorCode, Launch, LoaderVersion, McVersion};
use mh_net::{HostAllowlist, HttpClient, NetConfig, hosts};
use mh_providers::{Fabric, Quilt, Release};
use time::macros::datetime;
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture(name: &str) -> String {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap()
}

async fn serve(routes: &[(&str, String)]) -> (MockServer, tempfile::TempDir, Arc<HttpClient>) {
    let server = MockServer::start().await;
    for (route, body) in routes {
        Mock::given(path(*route))
            .respond_with(ResponseTemplate::new(200).set_body_string(body.clone()))
            .mount(&server)
            .await;
    }
    let tmp = tempfile::tempdir().unwrap();
    let mut config = NetConfig::new(tmp.path());
    config.retry_delays = vec![Duration::from_millis(1)];
    (server, tmp, Arc::new(HttpClient::new(config).unwrap()))
}

fn v(id: &str) -> McVersion {
    McVersion::parse(id).unwrap()
}

fn releases() -> Vec<Release> {
    [
        ("1.21.4", datetime!(2024-12-03 10:12 UTC)),
        ("1.20.1", datetime!(2023-06-12 13:25 UTC)),
        ("1.18.2", datetime!(2022-02-28 10:42 UTC)),
        ("1.14", datetime!(2019-04-23 14:52 UTC)),
        ("1.13.2", datetime!(2018-10-22 11:41 UTC)),
    ]
    .into_iter()
    .map(|(id, release_time)| Release {
        id: v(id),
        release_time,
    })
    .collect()
}

fn names(releases: &[Release]) -> Vec<&str> {
    releases.iter().map(|r| r.id.as_str()).collect()
}

#[tokio::test]
async fn fabric_versions_keep_stable_mojang_releases() {
    let (s, _t, http) = serve(&[("/v2/versions/game", fixture("fabric/game.json"))]).await;
    let fabric = Fabric::with_base_url(http, format!("{}/v2", s.uri()));
    let got = fabric.versions(&releases()).await.unwrap();
    assert_eq!(names(&got), ["1.21.4", "1.20.1", "1.14"]);
}

#[tokio::test]
async fn fabric_plan_uses_stable_loader_and_installer() {
    let (s, _t, http) = serve(&[(
        "/v2/versions/loader/1.21.4",
        fixture("fabric/loader-1.21.4.json"),
    )])
    .await;
    // Installer phải trỏ về server giả để tải được file `.sha256`.
    let installers =
        fixture("fabric/installer.json").replace("https://maven.fabricmc.net", &s.uri());
    Mock::given(path("/v2/versions/installer"))
        .respond_with(ResponseTemplate::new(200).set_body_string(installers))
        .mount(&s)
        .await;
    Mock::given(path(
        "/net/fabricmc/fabric-installer/1.1.2/fabric-installer-1.1.2.jar.sha256",
    ))
    .respond_with(
        ResponseTemplate::new(200)
            .set_body_string(fixture("fabric/fabric-installer-1.1.2.jar.sha256")),
    )
    .mount(&s)
    .await;
    let local = HostAllowlist::new(&["127.0.0.1"]);
    let fabric = Fabric::with_base_url(http, format!("{}/v2", s.uri())).with_hosts(local);

    let plan = fabric.plan(&v("1.21.4"), None).await.unwrap();
    assert_eq!(plan.loader_version.as_str(), "0.20.1");
    assert_eq!(plan.files.len(), 1);
    let file = &plan.files[0];
    assert!(
        file.url.ends_with("/fabric-installer-1.1.2.jar"),
        "{}",
        file.url
    );
    assert_eq!(
        file.checksum.hex(),
        "61e035bf7bf70153e127440ce34de47c9036f0a2d0c65d1529454bd35ceefe4f"
    );
    assert_eq!(file.hosts, local);
    assert_eq!(file.dest, ".minehost-install/fabric-installer-1.1.2.jar");
    let run = plan.installer.unwrap();
    assert_eq!(run.jar, file.dest);
    assert_eq!(
        run.args,
        [
            "server",
            "-mcversion",
            "1.21.4",
            "-loader",
            "0.20.1",
            "-downloadMinecraft"
        ]
    );
    let Launch::Jar(jar) = plan.launch else {
        panic!("Fabric chạy bằng -jar");
    };
    assert_eq!(jar.as_str(), "fabric-server-launch.jar");
}

#[tokio::test]
async fn fabric_unknown_loader_is_version_not_found() {
    let (s, _t, http) = serve(&[(
        "/v2/versions/loader/1.21.4",
        fixture("fabric/loader-1.21.4.json"),
    )])
    .await;
    let fabric = Fabric::with_base_url(http, format!("{}/v2", s.uri()));
    let wanted = LoaderVersion::parse("9.9.9").unwrap();
    let err = fabric.plan(&v("1.21.4"), Some(&wanted)).await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::VersionNotFound, "{err}");
}

#[tokio::test]
async fn quilt_loaders_are_sorted_with_stable_first() {
    let (s, _t, http) = serve(&[(
        "/v3/versions/loader/1.21.4",
        fixture("quilt/loader-1.21.4.json"),
    )])
    .await;
    let quilt = Quilt::with_base_url(http, format!("{}/v3", s.uri()));
    let got = quilt.loader_versions(&v("1.21.4")).await.unwrap();
    let names: Vec<_> = got.iter().map(|r| r.version.as_str()).collect();
    assert_eq!(names[0], "0.30.2", "{names:?}");
    assert!(got[0].stable);
    let betas: Vec<_> = got.iter().filter(|r| !r.stable).collect();
    assert!(betas.iter().all(|r| r.version.as_str().contains("-beta")));
    // Sau bản đề xuất, phần còn lại mới nhất trước.
    assert_eq!(names[1], "0.30.1", "{names:?}");
}

#[tokio::test]
async fn quilt_plan_uses_hash_from_meta() {
    let (s, _t, http) = serve(&[
        (
            "/v3/versions/loader/1.21.4",
            fixture("quilt/loader-1.21.4.json"),
        ),
        ("/v3/versions/installer", fixture("quilt/installer.json")),
        ("/v3/versions/game", fixture("quilt/game.json")),
    ])
    .await;
    let quilt = Quilt::with_base_url(http, format!("{}/v3", s.uri()));
    let got = quilt.versions(&releases()).await.unwrap();
    assert_eq!(names(&got), ["1.21.4", "1.20.1", "1.18.2"]);

    let plan = quilt.plan(&v("1.21.4"), None).await.unwrap();
    assert_eq!(plan.loader_version.as_str(), "0.30.2");
    let file = &plan.files[0];
    assert_eq!(
        file.checksum.hex(),
        "0a229138caa1b87fd8f5622038410696f98bb85871a279640e7002404c4d0dc2"
    );
    assert_eq!(file.size, Some(8_734_533));
    assert_eq!(
        plan.installer.unwrap().args,
        [
            "install",
            "server",
            "1.21.4",
            "0.30.2",
            "--download-server",
            "--install-dir=."
        ]
    );
    assert_eq!(
        plan.launch,
        Launch::Jar(mh_core::instance::JarName::parse("quilt-server-launch.jar").unwrap())
    );
}

/// Meta là dữ liệu bên ngoài: tên phiên bản thành tên file, URL thành nơi MineHost gọi tới.
#[tokio::test]
async fn hostile_installer_entries_are_refused() {
    let good = fixture("quilt/installer.json");
    let cases = [
        (good.replace("\"0.15.1\"", "\"../../evil\""), ErrorCode::NetworkUnavailable),
        (
            good.replace("https://maven.quiltmc.org", "https://evil.example"),
            ErrorCode::DownloadHostNotAllowed,
        ),
    ];
    for (installers, code) in cases {
        let (s, _t, http) = serve(&[
            ("/v3/versions/loader/1.21.4", fixture("quilt/loader-1.21.4.json")),
            ("/v3/versions/installer", installers),
        ])
        .await;
        let quilt = Quilt::with_base_url(http, format!("{}/v3", s.uri()));
        let err = quilt.plan(&v("1.21.4"), None).await.unwrap_err();
        assert_eq!(err.code(), code, "{err}");
    }
    assert_eq!(hosts::LOADERS.check_str("not a url").unwrap_err().code(), ErrorCode::DownloadHostNotAllowed);
}
