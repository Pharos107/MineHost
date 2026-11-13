//! Danh sách host được phép, SHA-512 và 429 (spec P3, mục 3.7 và 5.3); server giả `wiremock`.

#![allow(clippy::unwrap_used)]

use std::time::Duration;

use mh_core::ErrorCode;
use mh_net::{CancellationToken, Checksum, Download, HostAllowlist, HttpClient, NetConfig, NetError};
use sha2::{Digest, Sha512};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const LOCAL: HostAllowlist = HostAllowlist::new(&["127.0.0.1"]);
const BODY: &[u8] = b"noi dung mod";

fn client(cache: &tempfile::TempDir) -> HttpClient {
    let mut config = NetConfig::new(cache.path().join("cache"));
    config.retry_delays = vec![Duration::from_millis(1); 2];
    HttpClient::new(config).unwrap()
}

fn sha512() -> Checksum {
    let hex: String = Sha512::digest(BODY)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Checksum::sha512(&hex).unwrap()
}

async fn serve(server: &MockServer, route: &str, resp: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(resp)
        .mount(server)
        .await;
}

async fn fetch(
    tmp: &tempfile::TempDir,
    url: &str,
    allow: Option<&HostAllowlist>,
) -> Result<Vec<u8>, NetError> {
    let dest = tmp.path().join("mods/x.jar");
    let sum = sha512();
    let req = Download {
        url,
        checksum: &sum,
        dest: &dest,
        size: None,
        allowed_hosts: allow,
    };
    client(tmp)
        .download(req, |_, _| {}, &CancellationToken::new())
        .await?;
    Ok(std::fs::read(dest).unwrap())
}

#[tokio::test]
async fn listed_host_downloads_and_follows_same_host_redirect() {
    let server = MockServer::start().await;
    serve(&server, "/file.jar", ResponseTemplate::new(200).set_body_bytes(BODY)).await;
    serve(
        &server,
        "/old.jar",
        ResponseTemplate::new(302).insert_header("Location", "/file.jar"),
    )
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let url = format!("{}/old.jar", server.uri());
    assert_eq!(fetch(&tmp, &url, Some(&LOCAL)).await.unwrap(), BODY);
}

#[tokio::test]
async fn redirect_to_unlisted_host_is_refused() {
    let server = MockServer::start().await;
    // Cùng máy nhưng tên khác: `localhost` không có trong danh sách.
    let elsewhere = format!(
        "http://localhost:{}/file.jar",
        server.address().port()
    );
    serve(&server, "/file.jar", ResponseTemplate::new(200).set_body_bytes(BODY)).await;
    serve(
        &server,
        "/old.jar",
        ResponseTemplate::new(302).insert_header("Location", elsewhere.as_str()),
    )
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let err = fetch(&tmp, &format!("{}/old.jar", server.uri()), Some(&LOCAL))
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::DownloadHostNotAllowed, "{err}");
    assert_eq!(err.params()["host"], "localhost");
    assert!(!tmp.path().join("mods/x.jar").exists());
    assert!(!tmp.path().join("mods/x.jar.part").exists());
}

#[tokio::test]
async fn unlisted_url_is_refused_before_any_request() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let url = format!("{}/file.jar", server.uri());
    let only_cdn = HostAllowlist::new(&["cdn.modrinth.com"]);
    let err = fetch(&tmp, &url, Some(&only_cdn)).await.unwrap_err();
    assert!(matches!(err, NetError::HostNotAllowed { .. }), "{err}");
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn long_rate_limit_is_reported_with_wait() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/file.jar",
        ResponseTemplate::new(429).insert_header("Retry-After", "60"),
    )
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let err = fetch(&tmp, &format!("{}/file.jar", server.uri()), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::UpstreamRateLimited);
    assert_eq!(err.params()["retry_after_s"], 60);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn short_rate_limit_waits_once_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/file.jar"))
        .respond_with(ResponseTemplate::new(429).insert_header("X-Ratelimit-Reset", "0"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    serve(&server, "/file.jar", ResponseTemplate::new(200).set_body_bytes(BODY)).await;
    let tmp = tempfile::tempdir().unwrap();
    let got = fetch(&tmp, &format!("{}/file.jar", server.uri()), Some(&LOCAL)).await;
    assert_eq!(got.unwrap(), BODY);
}

#[tokio::test]
async fn second_rate_limit_is_not_retried_again() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/file.jar",
        ResponseTemplate::new(429).insert_header("Retry-After", "0"),
    )
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let err = fetch(&tmp, &format!("{}/file.jar", server.uri()), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::UpstreamRateLimited);
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[test]
fn sha512_needs_128_hex_digits() {
    assert!(Checksum::sha512(&"a".repeat(128)).is_some());
    assert!(Checksum::sha512(&"a".repeat(64)).is_none());
    assert!(Checksum::sha512(&"g".repeat(128)).is_none());
}
