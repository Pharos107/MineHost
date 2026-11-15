//! Trần số byte khi tải và file tạm riêng cho từng lần ghi.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use mh_core::ErrorCode;
use mh_net::{CancellationToken, Checksum, Download, HttpClient, NetConfig};
use sha2::{Digest, Sha256};
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sha256_of(data: &[u8]) -> Checksum {
    let hex: String = Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Checksum::sha256(&hex).unwrap()
}

async fn serve(body: &[u8]) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(path("/f"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn stops_when_source_sends_more_than_declared() {
    let body = vec![7u8; 4096];
    let server = serve(&body).await;
    let tmp = tempfile::tempdir().unwrap();
    let http = HttpClient::new(NetConfig::new(tmp.path().join("cache"))).unwrap();
    let dest = tmp.path().join("out/big.jar");
    let sum = sha256_of(&body);
    let req = Download {
        url: &format!("{}/f", server.uri()),
        checksum: &sum,
        dest: &dest,
        size: Some(1000),
        allowed_hosts: None,
    };

    let err = http
        .download(req, |_, _| {}, &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::DownloadFailed);
    assert!(!dest.exists());
    assert!(!tmp.path().join("out/big.jar.part").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_copies_from_cache_do_not_share_temp_files() {
    let body: Vec<u8> = (0..200_000).map(|i| (i % 251) as u8).collect();
    let server = serve(&body).await;
    let tmp = tempfile::tempdir().unwrap();
    let http = Arc::new(HttpClient::new(NetConfig::new(tmp.path().join("cache"))).unwrap());
    let url = format!("{}/f", server.uri());
    let sum = sha256_of(&body);
    let out = tmp.path().join("out");
    // Tải một lần để các lượt sau chép từ cache.
    let seed = tmp.path().join("seed.bin");
    let req = Download {
        url: &url,
        checksum: &sum,
        dest: &seed,
        size: None,
        allowed_hosts: None,
    };
    http.download(req, |_, _| {}, &CancellationToken::new())
        .await
        .unwrap();

    // Cùng tên gốc, khác đuôi: trước đây cùng ghi vào `out/foo.tmp`.
    let names = [
        "foo.json", "foo.toml", "foo.cfg", "foo.txt", "foo.jar", "foo.zip",
    ];
    let mut set = tokio::task::JoinSet::new();
    for name in names {
        let (http, url, sum, dest) = (http.clone(), url.clone(), sum.clone(), out.join(name));
        set.spawn(async move {
            let req = Download {
                url: &url,
                checksum: &sum,
                dest: &dest,
                size: Some(200_000),
                allowed_hosts: None,
            };
            http.download(req, |_, _| {}, &CancellationToken::new())
                .await
        });
    }
    while let Some(res) = set.join_next().await {
        res.unwrap().unwrap();
    }
    for name in names {
        assert_eq!(std::fs::read(out.join(name)).unwrap(), body, "{name}");
    }
    let leftovers: Vec<_> = std::fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp") || n.ends_with(".part"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}
