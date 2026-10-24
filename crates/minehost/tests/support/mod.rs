//! Điều khiển binary `minehost` thật qua HTTP cho test tích hợp với Paper (spec P1, mục 9).
//!
//! Thư mục dữ liệu lấy từ `MINEHOST_E2E_DIR` để CI cache được `java/` và `cache/` giữa các lần
//! chạy; không có thì dùng thư mục tạm. Mỗi test xóa `servers/` trước khi chạy.

#![allow(clippy::unwrap_used, clippy::panic, clippy::print_stderr, dead_code)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// Phiên bản cố định để CI ổn định và cache được (spec P1, mục 9).
pub const MINECRAFT: &str = "1.21.4";

pub struct DataDir {
    path: PathBuf,
    _tmp: Option<tempfile::TempDir>,
}

impl DataDir {
    pub fn prepare() -> Self {
        let (path, tmp) = match std::env::var_os("MINEHOST_E2E_DIR") {
            Some(dir) => (PathBuf::from(dir), None),
            None => {
                let tmp = tempfile::tempdir().unwrap();
                (tmp.path().to_path_buf(), Some(tmp))
            }
        };
        std::fs::create_dir_all(&path).unwrap();
        for dir in ["servers", "trash", "logs"] {
            let _ = std::fs::remove_dir_all(path.join(dir));
        }
        // Cổng 0: hệ điều hành chọn cổng trống, không đụng MineHost thật ở 7878.
        std::fs::write(
            path.join("config.toml"),
            "listen = \"127.0.0.1:0\"\nopen_browser = false\n",
        )
        .unwrap();
        Self { path, _tmp: tmp }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub struct Controller {
    child: Option<Child>,
    pub url: String,
    dir: PathBuf,
}

impl Controller {
    pub fn start(dir: &DataDir) -> Self {
        // `runtime.json` còn lại sau khi test kill controller; xóa để không đọc nhầm địa chỉ cũ.
        let _ = std::fs::remove_file(dir.path().join("runtime.json"));
        let child = Command::new(env!("CARGO_BIN_EXE_minehost"))
            .arg("--data-dir")
            .arg(dir.path())
            .args(["--no-browser", "--no-tray"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut controller = Self {
            child: Some(child),
            url: String::new(),
            dir: dir.path().to_path_buf(),
        };
        controller.url = controller.wait_for_url();
        controller
    }

    fn wait_for_url(&mut self) -> String {
        let path = self.dir.join("runtime.json");
        wait_until(
            "controller ghi runtime.json",
            Duration::from_secs(60),
            || {
                let text = std::fs::read_to_string(&path).ok()?;
                let json: Value = serde_json::from_str(&text).ok()?;
                json["url"].as_str().map(str::to_owned)
            },
        )
    }

    pub fn get(&self, path: &str) -> Value {
        let (status, body) = http(&self.url, "GET", path, None);
        assert_eq!(status, 200, "GET {path}: {body}");
        body
    }

    pub fn post(&self, path: &str, body: Value) -> Value {
        let (status, body) = http(&self.url, "POST", path, Some(body));
        assert!((200..300).contains(&status), "POST {path}: {status} {body}");
        body
    }

    /// Tạo server Paper RAM nhỏ (CI chạy tới 3 server cùng lúc) và chờ Job cài xong.
    pub fn create_server(&self, name: &str) -> String {
        let created = self.post(
            "/servers",
            json!({ "name": name, "loader": "paper", "minecraft": MINECRAFT,
                    "memory_mb": { "min": 512, "max": 1024 } }),
        );
        let job = created["job_id"].as_u64().unwrap();
        let state = wait_until("Job tạo server xong", Duration::from_secs(900), || {
            let job = self.get(&format!("/jobs/{job}"));
            (job["state"] != "running").then_some(job)
        });
        assert_eq!(state["state"], "succeeded", "Job lỗi: {state}");
        created["server_id"].as_str().unwrap().to_owned()
    }

    /// Đồng ý EULA rồi bật, chờ tới khi thấy dòng `Done`. Trả về pid của Java.
    pub fn start_server(&self, id: &str) -> u32 {
        self.post(&format!("/servers/{id}/eula"), json!({ "accept": true }));
        self.post(&format!("/servers/{id}/actions/start"), json!({}));
        let server = self.wait_state(id, "running", Duration::from_secs(300));
        u32::try_from(server["pid"].as_u64().unwrap()).unwrap()
    }

    pub fn wait_state(&self, id: &str, state: &str, limit: Duration) -> Value {
        wait_until(&format!("server {id} sang {state}"), limit, || {
            let server = self.get(&format!("/servers/{id}"));
            assert_ne!(server["state"], "crashed", "server crash: {server}");
            (server["state"] == state).then_some(server)
        })
    }

    /// Chờ một dòng console chứa `needle`, trả về dòng đó.
    pub fn wait_console(&self, id: &str, needle: &str) -> String {
        wait_until(
            &format!("console có `{needle}`"),
            Duration::from_secs(60),
            || {
                let console = self.get(&format!("/servers/{id}/console"));
                console["lines"].as_array()?.iter().find_map(|line| {
                    let text = line["text"].as_str()?;
                    text.contains(needle).then(|| text.to_owned())
                })
            },
        )
    }

    /// Bấm Thoát qua API rồi chờ controller tự tắt.
    pub fn shutdown(mut self, limit: Duration) -> ExitStatus {
        let (status, body) = http(&self.url, "POST", "/app/shutdown", None);
        assert_eq!(status, 202, "{body}");
        let mut child = self.child.take().unwrap();
        wait_until("controller thoát", limit, || child.try_wait().unwrap())
    }

    /// Giống controller bị crash hoặc bị kill -9 từ Task Manager.
    pub fn kill(mut self) {
        let mut child = self.child.take().unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
    }

    pub fn log(&self) -> String {
        let mut text = String::new();
        if let Ok(entries) = std::fs::read_dir(self.dir.join("logs")) {
            for entry in entries.flatten() {
                text += &std::fs::read_to_string(entry.path()).unwrap_or_default();
            }
        }
        text
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        if std::thread::panicking() {
            // In log của controller để CI cho biết vì sao test hỏng.
            let log = self.log();
            let tail: Vec<&str> = log.lines().rev().take(80).collect();
            eprintln!("--- minehost.log (cuối) ---");
            for line in tail.iter().rev() {
                eprintln!("{line}");
            }
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub fn wait_until<T>(what: &str, limit: Duration, mut check: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(value) = check() {
            return value;
        }
        assert!(
            start.elapsed() < limit,
            "quá {limit:?} mà chưa thấy: {what}"
        );
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Process còn sống không, để kiểm tra không còn Java mồ côi.
pub fn process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\""))
    }
    #[cfg(unix)]
    {
        // Process zombie chưa được thu dọn vẫn có trong /proc nhưng đã chết.
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .map(|stat| !stat.contains(") Z "))
            .unwrap_or(false)
    }
}

/// HTTP/1.1 tối giản qua `TcpStream`, đủ cho JSON của API; tránh thêm client HTTP vào test.
fn http(base: &str, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
    let host = base.trim_start_matches("http://");
    let mut stream = TcpStream::connect(host).unwrap();
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let content_type = if body.is_empty() {
        ""
    } else {
        "Content-Type: application/json\r\n"
    };
    let request = format!(
        "{method} /api/v1{path} HTTP/1.1\r\nHost: {host}\r\nX-MineHost: 1\r\n{content_type}\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (head, payload) = response.split_once("\r\n\r\n").unwrap_or((&response, ""));
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (status, serde_json::from_str(payload).unwrap_or(Value::Null))
}
