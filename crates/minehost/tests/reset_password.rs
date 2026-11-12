//! `minehost reset-password` khi controller đang chạy (spec P2, mục 3.3; ADR 0003).

#![allow(clippy::unwrap_used)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn minehost(data_dir: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_minehost"));
    cmd.arg("--data-dir")
        .arg(data_dir)
        .args(["--no-browser", "--no-tray"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd
}

struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_for_url(data_dir: &Path) -> String {
    let start = Instant::now();
    loop {
        if let Ok(text) = std::fs::read_to_string(data_dir.join("runtime.json"))
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&text)
            && let Some(url) = json["url"].as_str()
        {
            return url.to_owned();
        }
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "không có runtime.json"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// HTTP tay để test không cần thêm client. Trả về dòng trạng thái và body.
fn request(url: &str, method: &str, path: &str, body: &str) -> (String, String) {
    let host = url.trim_start_matches("http://");
    let mut stream = TcpStream::connect(host).unwrap();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nX-MineHost: 1\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let status = response.lines().next().unwrap_or_default().to_owned();
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_owned())
        .unwrap_or_default();
    (status, body)
}

#[test]
fn reset_password_while_running_unlocks_next_request() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    std::fs::write(
        dir.join("config.toml"),
        "listen = \"127.0.0.1:0\"\nopen_browser = false\n",
    )
    .unwrap();
    let mut controller = KillOnDrop(minehost(dir).spawn().unwrap());
    let url = wait_for_url(dir);

    let (status, _) = request(
        &url,
        "PUT",
        "/api/v1/auth/password",
        r#"{"new":"correct-horse"}"#,
    );
    assert!(status.starts_with("HTTP/1.1 204"), "{status}");
    let (status, body) = request(&url, "GET", "/api/v1/servers", "");
    assert!(status.starts_with("HTTP/1.1 401"), "{status} {body}");

    let output = minehost(dir)
        .arg("reset-password")
        .stdout(Stdio::piped())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("Đã xóa mật khẩu"));

    let (status, body) = request(&url, "GET", "/api/v1/servers", "");
    assert!(status.starts_with("HTTP/1.1 200"), "{status} {body}");
    let (_, body) = request(&url, "GET", "/api/v1/auth/session", "");
    assert!(body.contains("\"password_set\":false"), "{body}");

    let (status, _) = request(&url, "POST", "/api/v1/app/shutdown", "");
    assert!(status.starts_with("HTTP/1.1 202"), "{status}");
    let start = Instant::now();
    while controller.0.try_wait().unwrap().is_none() {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "controller không tắt"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}
