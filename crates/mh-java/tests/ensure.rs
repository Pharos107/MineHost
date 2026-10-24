//! Test `JavaManager::ensure` với API Adoptium giả và JRE giả; không cần Java thật.

// Hàm trợ giúp ngoài `#[test]` không được clippy coi là code test.
#![allow(clippy::unwrap_used)]

use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use mh_core::ErrorCode;
use mh_java::{JavaManager, ProbeFn, java_bin};
use mh_net::{CancellationToken, HttpClient, NetConfig};
use sha2::{Digest, Sha256};
use wiremock::matchers::{path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// JRE giả có cả `java` lẫn `java.exe`, để test giống nhau trên Windows và Linux.
fn fake_jre_zip(entries: &[&str]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buf);
    for name in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"fake").unwrap();
    }
    zip.finish().unwrap();
    buf.into_inner()
}

const GOOD: &[&str] = &[
    "jdk-21.0.5+11-jre/bin/java",
    "jdk-21.0.5+11-jre/bin/java.exe",
    "jdk-21.0.5+11-jre/release",
];

/// Server giả trả JSON theo đúng cấu trúc của fixture thật, trỏ link tới archive giả.
async fn adoptium(archive: Vec<u8>, expect: u64) -> MockServer {
    let server = MockServer::start().await;
    let checksum: String = Sha256::digest(&archive)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let fixture = format!(
        "{}/fixtures/adoptium-21-windows-x64.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut assets: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture).unwrap()).unwrap();
    let package = &mut assets[0]["binary"]["package"];
    package["link"] = format!("{}/jre.zip", server.uri()).into();
    package["checksum"] = checksum.into();
    package["size"] = archive.len().into();
    Mock::given(path("/v3/assets/latest/21/hotspot"))
        .and(query_param("image_type", "jre"))
        .and(query_param("vendor", "eclipse"))
        .respond_with(ResponseTemplate::new(200).set_body_json(assets))
        .expect(expect)
        .mount(&server)
        .await;
    Mock::given(path("/jre.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive))
        .expect(expect)
        .mount(&server)
        .await;
    server
}

/// Thay `java -version`: chỉ cần file `java` tồn tại.
fn fake_probe() -> ProbeFn {
    Arc::new(|bin: &Path| {
        if bin.is_file() {
            Ok("21.0.5".to_owned())
        } else {
            Err(format!("không có {}", bin.display()))
        }
    })
}

fn manager(server: &MockServer, data: &Path, probe: ProbeFn) -> JavaManager {
    let mut config = NetConfig::new(data.join("cache"));
    config.retry_delays = vec![Duration::from_millis(1)];
    let http = Arc::new(HttpClient::new(config).unwrap());
    JavaManager::new(http, data.join("java"))
        .with_api_base(server.uri())
        .with_probe(probe)
}

fn leftovers(java_dir: &Path) -> Vec<String> {
    std::fs::read_dir(java_dir)
        .map(|it| {
            it.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn installs_into_java_major_without_root_directory() {
    let server = adoptium(fake_jre_zip(GOOD), 1).await;
    let tmp = tempfile::tempdir().unwrap();
    let java = manager(&server, tmp.path(), fake_probe());

    let rt = java
        .ensure(21, |_, _| {}, &CancellationToken::new())
        .await
        .unwrap();
    let home = tmp.path().join("java").join("21");
    assert_eq!(rt.home, home);
    assert_eq!(rt.java_bin, java_bin(&home));
    assert_eq!(rt.version, "21.0.5");
    assert!(home.join("release").is_file());
    assert_eq!(leftovers(&tmp.path().join("java")), ["21"]);
}

#[tokio::test]
async fn concurrent_ensure_downloads_once() {
    // `expect(1)`: nếu tải hai lần, wiremock báo lỗi khi server bị hủy cuối test.
    let server = adoptium(fake_jre_zip(GOOD), 1).await;
    let tmp = tempfile::tempdir().unwrap();
    let java = manager(&server, tmp.path(), fake_probe());
    let cancel = CancellationToken::new();

    let (a, b) = tokio::join!(
        java.ensure(21, |_, _| {}, &cancel),
        java.ensure(21, |_, _| {}, &cancel)
    );
    assert_eq!(a.unwrap().home, b.unwrap().home);
    // Lần gọi sau đó dùng bản đã cài, không gọi API.
    java.ensure(21, |_, _| {}, &cancel).await.unwrap();
}

#[tokio::test]
async fn evil_archive_is_blocked_and_cleaned_up() {
    let server = adoptium(fake_jre_zip(&["jre/bin/java", "../evil"]), 1).await;
    let tmp = tempfile::tempdir().unwrap();
    let java = manager(&server, tmp.path(), fake_probe());

    let err = java
        .ensure(21, |_, _| {}, &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::JavaInstallFailed);
    assert!(leftovers(&tmp.path().join("java")).is_empty());
    assert!(!tmp.path().join("evil").exists());
}

#[tokio::test]
async fn failing_java_is_not_installed() {
    let server = adoptium(fake_jre_zip(GOOD), 1).await;
    let tmp = tempfile::tempdir().unwrap();
    let java = manager(
        &server,
        tmp.path(),
        Arc::new(|_: &Path| Err("crash".to_owned())),
    );

    let err = java
        .ensure(21, |_, _| {}, &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::JavaInstallFailed);
    assert!(leftovers(&tmp.path().join("java")).is_empty());
}

#[tokio::test]
async fn broken_existing_install_is_replaced() {
    let server = adoptium(fake_jre_zip(GOOD), 1).await;
    let tmp = tempfile::tempdir().unwrap();
    let broken = tmp.path().join("java").join("21");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("garbage"), b"x").unwrap();
    let java = manager(&server, tmp.path(), fake_probe());

    java.ensure(21, |_, _| {}, &CancellationToken::new())
        .await
        .unwrap();
    assert!(broken.join("release").is_file());
    assert!(!broken.join("garbage").exists());
}

/// Tải JRE thật (~50 MB): `cargo test -p mh-java -- --ignored`.
#[tokio::test]
#[ignore = "tải JRE thật từ Adoptium"]
async fn real_download_runs_java_version() {
    let tmp = tempfile::tempdir().unwrap();
    let http = Arc::new(HttpClient::new(NetConfig::new(tmp.path().join("cache"))).unwrap());
    let java = JavaManager::new(http, tmp.path().join("java"));
    let rt = java
        .ensure(21, |_, _| {}, &CancellationToken::new())
        .await
        .unwrap();
    assert!(rt.version.starts_with("21"), "{}", rt.version);
}
