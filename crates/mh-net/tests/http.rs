//! Test `mh-net` với server giả `wiremock`; không gọi mạng thật.

// Hàm trợ giúp ngoài `#[test]` không được clippy coi là code test.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use mh_core::ErrorCode;
use mh_net::{CancellationToken, Checksum, Download, HttpClient, NetConfig, USER_AGENT};
use sha2::{Digest, Sha256};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TTL: Duration = Duration::from_secs(60);

fn client(cache: &tempfile::TempDir) -> HttpClient {
    let mut config = NetConfig::new(cache.path());
    config.retry_delays = vec![Duration::from_millis(1); 3];
    HttpClient::new(config).unwrap()
}

fn sha256_of(data: &[u8]) -> Checksum {
    let hex: String = Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Checksum::sha256(&hex).unwrap()
}

async fn mock(server: &MockServer, route: &str, status: u16, body: &str, times: Option<u64>) {
    let mut m = Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(status).set_body_string(body));
    if let Some(n) = times {
        m = m.up_to_n_times(n);
    }
    m.mount(server).await;
}

#[tokio::test]
async fn retries_on_503_then_succeeds() {
    let server = MockServer::start().await;
    mock(&server, "/v", 503, "", Some(2)).await;
    mock(&server, "/v", 200, r#"{"a":1}"#, None).await;
    let tmp = tempfile::tempdir().unwrap();

    let got = client(&tmp)
        .get_json::<serde_json::Value>(&format!("{}/v", server.uri()), TTL)
        .await
        .unwrap();
    assert_eq!(got.value["a"], 1);
    assert!(!got.stale);
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn gives_up_after_all_retries() {
    let server = MockServer::start().await;
    mock(&server, "/v", 503, "", None).await;
    let tmp = tempfile::tempdir().unwrap();

    let err = client(&tmp)
        .get_json::<serde_json::Value>(&format!("{}/v", server.uri()), TTL)
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::NetworkUnavailable);
    // Một lần đầu và ba lần thử lại.
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
}

#[tokio::test]
async fn not_found_is_not_retried() {
    let server = MockServer::start().await;
    mock(&server, "/v", 404, "", None).await;
    let tmp = tempfile::tempdir().unwrap();

    let err = client(&tmp)
        .get_json::<serde_json::Value>(&format!("{}/v", server.uri()), TTL)
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn sends_user_agent_with_contact() {
    let server = MockServer::start().await;
    Mock::given(header("user-agent", USER_AGENT))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    client(&tmp)
        .get_json::<Vec<u8>>(&server.uri(), TTL)
        .await
        .unwrap();
    assert!(USER_AGENT.contains("github.com/Pharos107/MineHost"));
}

#[tokio::test]
async fn json_is_cached_in_memory() {
    let server = MockServer::start().await;
    Mock::given(path("/v"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[1]"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let http = client(&tmp);
    let url = format!("{}/v", server.uri());

    for _ in 0..2 {
        let got = http.get_json::<Vec<u8>>(&url, TTL).await.unwrap();
        assert_eq!(got.value, [1]);
    }
}

#[tokio::test]
async fn falls_back_to_last_good_copy_when_offline() {
    let server = MockServer::start().await;
    mock(&server, "/v", 200, "[7]", Some(1)).await;
    mock(&server, "/v", 503, "", None).await;
    let tmp = tempfile::tempdir().unwrap();
    let url = format!("{}/v", server.uri());

    client(&tmp).get_json::<Vec<u8>>(&url, TTL).await.unwrap();
    // Client mới (giống lần mở MineHost sau) không còn cache RAM, chỉ còn `cache/meta/`.
    let got = client(&tmp).get_json::<Vec<u8>>(&url, TTL).await.unwrap();
    assert_eq!(got.value, [7]);
    assert!(got.stale);
}

#[tokio::test]
async fn bad_json_is_reported_and_not_cached() {
    let server = MockServer::start().await;
    mock(&server, "/v", 200, "{not json", None).await;
    let tmp = tempfile::tempdir().unwrap();

    let err = client(&tmp)
        .get_json::<Vec<u8>>(&format!("{}/v", server.uri()), TTL)
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::NetworkUnavailable);
    assert!(!tmp.path().join("meta").exists());
}

struct Fixture {
    server: MockServer,
    tmp: tempfile::TempDir,
    body: Vec<u8>,
}

async fn file_server(body_len: usize, expect: u64) -> Fixture {
    let server = MockServer::start().await;
    let body: Vec<u8> = (0..body_len).map(|i| (i % 251) as u8).collect();
    Mock::given(path("/server.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
        .expect(expect)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    Fixture { server, tmp, body }
}

impl Fixture {
    fn url(&self) -> String {
        format!("{}/server.jar", self.server.uri())
    }

    fn dest(&self, name: &str) -> std::path::PathBuf {
        self.tmp.path().join("servers").join(name)
    }
}

#[tokio::test]
async fn downloads_and_verifies_hash() {
    let fx = file_server(300_000, 1).await;
    let sum = sha256_of(&fx.body);
    let dest = fx.dest("server.jar");
    let mut last = (0, None);
    let req = Download {
        url: &fx.url(),
        checksum: &sum,
        dest: &dest,
        size: None,
    };

    client(&fx.tmp)
        .download(req, |d, t| last = (d, t), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), fx.body);
    assert_eq!(last, (300_000, Some(300_000)));
    assert!(!fx.dest("server.jar.part").exists());
}

#[tokio::test]
async fn wrong_hash_removes_part_file() {
    let fx = file_server(1000, 1).await;
    let sum = sha256_of(b"something else");
    let dest = fx.dest("server.jar");
    let req = Download {
        url: &fx.url(),
        checksum: &sum,
        dest: &dest,
        size: None,
    };

    let err = client(&fx.tmp)
        .download(req, |_, _| {}, &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::ChecksumMismatch);
    assert!(!dest.exists());
    assert!(!fx.dest("server.jar.part").exists());
}

#[tokio::test]
async fn cancel_midway_leaves_nothing() {
    let fx = file_server(2_000_000, 1).await;
    let sum = sha256_of(&fx.body);
    let dest = fx.dest("server.jar");
    let cancel = CancellationToken::new();
    let req = Download {
        url: &fx.url(),
        checksum: &sum,
        dest: &dest,
        size: None,
    };

    let err = client(&fx.tmp)
        .download(req, |_, _| cancel.cancel(), &cancel)
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::JobCancelled);
    assert!(!dest.exists());
    assert!(!fx.dest("server.jar.part").exists());
}

#[tokio::test]
async fn second_download_comes_from_cache() {
    // `expect(1)`: lần tải thứ hai mà gọi mạng thì wiremock báo lỗi khi kết thúc test.
    let fx = file_server(5000, 1).await;
    let sum = sha256_of(&fx.body);
    let http = client(&fx.tmp);

    for name in ["a.jar", "b.jar"] {
        let dest = fx.dest(name);
        let req = Download {
            url: &fx.url(),
            checksum: &sum,
            dest: &dest,
            size: None,
        };
        http.download(req, |_, _| {}, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), fx.body);
    }
}

#[tokio::test]
async fn refuses_when_disk_is_almost_full() {
    let fx = file_server(10, 0).await;
    let sum = sha256_of(&fx.body);
    let dest = fx.dest("server.jar");
    let http = client(&fx.tmp).with_free_space(Arc::new(|_| Some(10 * 1024 * 1024)));
    let req = Download {
        url: &fx.url(),
        checksum: &sum,
        dest: &dest,
        size: None,
    };

    let err = http
        .download(req, |_, _| {}, &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotEnoughDisk);
}

#[test]
fn checksum_requires_hex_of_right_length() {
    assert!(Checksum::sha1(&"a".repeat(40)).is_some());
    assert!(Checksum::sha1(&"a".repeat(64)).is_none());
    assert!(Checksum::sha256(&"../".repeat(21)).is_none());
    assert_eq!(
        Checksum::sha1(&"AB".repeat(20)).unwrap().hex(),
        "ab".repeat(20)
    );
}
