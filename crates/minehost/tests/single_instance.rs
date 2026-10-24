//! Chạy binary thật: bản thứ hai thoát ngay, bản đầu tắt sạch khi nhận lệnh Thoát.

#![allow(clippy::unwrap_used)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
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

/// Kill process nếu test dừng giữa chừng, để không để lại controller chạy nền.
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_exit(child: &mut Child, limit: Duration) -> ExitStatus {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(start.elapsed() < limit, "process không thoát sau {limit:?}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn wait_for_url(data_dir: &Path) -> String {
    let path = data_dir.join("runtime.json");
    let start = Instant::now();
    loop {
        if let Ok(text) = std::fs::read_to_string(&path)
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&text)
            && let Some(url) = json["url"].as_str()
        {
            return url.to_owned();
        }
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "MineHost không ghi runtime.json"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Gửi `POST /api/v1/app/shutdown` bằng HTTP tay, để test không cần thêm client HTTP.
fn request_shutdown(url: &str) -> String {
    let host = url.trim_start_matches("http://");
    let mut stream = TcpStream::connect(host).unwrap();
    let request = format!(
        "POST /api/v1/app/shutdown HTTP/1.1\r\nHost: {host}\r\nX-MineHost: 1\r\n\
         Content-Length: 0\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn second_instance_exits_and_first_shuts_down_cleanly() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    // Cổng 0: hệ điều hành chọn cổng trống, test không đụng MineHost thật đang chạy ở 7878.
    std::fs::write(
        dir.join("config.toml"),
        "listen = \"127.0.0.1:0\"\nopen_browser = false\n",
    )
    .unwrap();

    let mut first = KillOnDrop(minehost(dir).spawn().unwrap());
    let url = wait_for_url(dir);

    let mut second = KillOnDrop(minehost(dir).spawn().unwrap());
    let status = wait_exit(&mut second.0, Duration::from_secs(20));
    assert!(
        status.success(),
        "bản thứ hai phải thoát mã 0, được {status}"
    );
    assert!(
        first.0.try_wait().unwrap().is_none(),
        "bản đầu phải vẫn chạy"
    );
    assert_eq!(
        wait_for_url(dir),
        url,
        "bản thứ hai không được ghi đè địa chỉ"
    );

    let response = request_shutdown(&url);
    assert!(response.starts_with("HTTP/1.1 202"), "{response}");
    let status = wait_exit(&mut first.0, Duration::from_secs(30));
    assert!(status.success(), "bản đầu phải tắt sạch, được {status}");
    assert!(!dir.join("runtime.json").exists());
}
