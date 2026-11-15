//! Test modpack hằng đêm (spec P3 mục 9.2): mỗi pack trong `tests/modpacks.toml` được cài qua API
//! modpack, bật tới `Done` (preflight phải qua, không dùng `ignore_preflight`) rồi tắt sạch.
//! Chạy trên runner tự host (`docs/ops/runner-modpack.md`) bằng workflow `modpacks`; mặc định bị
//! `#[ignore]` vì tải hàng GB và cần tới 10 GB RAM mỗi pack.
//!
//! Biến môi trường:
//! - `MINEHOST_E2E_DIR`: thư mục dữ liệu, giữ cache Java và file modpack giữa các đêm.
//! - `MINEHOST_MODPACKS`: chỉ chạy các pack có `id` trong danh sách này (cách nhau bằng dấu phẩy).
//! - `MINEHOST_MODPACK_REPORT`: file JSON Lines nhận kết quả từng pack (artifact của workflow).
//! - `MINEHOST_MODPACK_IGNORE_PREFLIGHT=1`: chỉ để chẩn đoán khi preflight chặn một pack: bật bất
//!   chấp preflight để xem loader thật có từ chối không (chặn đúng) hay vẫn tới `Done` (chặn sai).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stderr,
    clippy::panic
)]

mod support;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};

use support::{Controller, DataDir, process_alive, wait_until};

/// Cổng game của server test, tránh 25565 và các cổng của dịch vụ khác trên máy runner.
const GAME_PORT: u16 = 25610;
/// Cổng cố định của controller: luật nftables của runner chỉ mở loopback cho hai cổng này
/// (`scripts/modpack-runner.sh`), nên không dùng cổng 0 như các test tích hợp khác.
const CONTROLLER_PORT: u16 = 25611;

#[derive(Deserialize)]
struct PackList {
    pack: Vec<Pack>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pack {
    id: String,
    name: String,
    source: String,
    project_id: String,
    version_id: String,
    loader: String,
    minecraft: String,
    memory_mb: u32,
    done_timeout_s: u64,
    #[serde(default = "default_install_timeout")]
    install_timeout_s: u64,
    #[serde(default)]
    note: String,
    /// Pack có mod CurseForge cấm phân phối: test đóng vai người dùng tải tay (spec P3 mục 9.2).
    #[serde(default)]
    manual_downloads: bool,
}

fn default_install_timeout() -> u64 {
    1800
}

fn load_packs() -> Vec<Pack> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/modpacks.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let list: PackList = toml::from_str(&text).unwrap();
    let only: Option<Vec<String>> = std::env::var("MINEHOST_MODPACKS")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.split(',').map(|p| p.trim().to_owned()).collect());
    list.pack
        .into_iter()
        .filter(|p| only.as_ref().is_none_or(|ids| ids.contains(&p.id)))
        .collect()
}

/// Kết quả của một pack, ghi vào báo cáo.
#[derive(Default)]
struct Outcome {
    status: &'static str,
    install_s: f64,
    done_s: f64,
    peak_rss_mb: u64,
    mods: u64,
    skipped_client: u64,
    error: String,
}

fn run_pack(pack: &Pack) -> Outcome {
    let data = DataDir::prepare();
    std::fs::write(
        data.path().join("config.toml"),
        format!("listen = \"127.0.0.1:{CONTROLLER_PORT}\"\nopen_browser = false\n"),
    )
    .unwrap();
    let controller = Controller::start(&data);
    if pack.source == "curseforge" && controller.get("/settings/curseforge")["available"] != true {
        return Outcome {
            status: "skipped",
            error: "chưa có key CurseForge (secret MINEHOST_CURSEFORGE_KEY)".to_owned(),
            ..Outcome::default()
        };
    }
    let mut out = Outcome::default();

    let started = Instant::now();
    let created = controller.post(
        "/servers/modpack",
        json!({ "source": pack.source, "project_id": pack.project_id,
                "version_id": pack.version_id, "name": pack.name, "port": GAME_PORT,
                "memory_mb": { "min": pack.memory_mb.min(4096), "max": pack.memory_mb } }),
    );
    let id = created["server_id"].as_str().unwrap().to_owned();
    let job = wait_job(&controller, created["job_id"].as_u64().unwrap(), pack);
    let pending = &job["result"]["pending_curseforge"];
    if pending.as_array().is_some_and(|p| !p.is_empty()) {
        assert!(
            pack.manual_downloads,
            "còn file CurseForge chờ tải tay: {pending}"
        );
        supply_manual_files(&controller, &data, &id);
    }
    out.install_s = started.elapsed().as_secs_f64();
    out.skipped_client = job["result"]["skipped_client"]
        .as_array()
        .map_or(0, |a| a.len() as u64);
    let server = controller.get(&format!("/servers/{id}"));
    assert_eq!(server["loader"], pack.loader.as_str(), "sai loader");
    assert_eq!(
        server["minecraft"],
        pack.minecraft.as_str(),
        "sai Minecraft"
    );

    let started = Instant::now();
    controller.post(&format!("/servers/{id}/eula"), json!({ "accept": true }));
    let ignore = std::env::var_os("MINEHOST_MODPACK_IGNORE_PREFLIGHT").is_some_and(|v| v == "1");
    controller.post(
        &format!("/servers/{id}/actions/start"),
        json!({ "ignore_preflight": ignore }),
    );
    let running = controller.wait_state(&id, "running", Duration::from_secs(pack.done_timeout_s));
    out.done_s = started.elapsed().as_secs_f64();
    let pid = u32::try_from(running["pid"].as_u64().unwrap()).unwrap();
    out.peak_rss_mb = peak_rss_mb(pid);
    // Bắt lỗi kiểu server pack chỉ có script tải mod lúc chạy: server vẫn lên mà không có mod.
    out.mods = count_jars(&data.path().join("servers").join(&id).join("server/mods"));
    assert!(out.mods > 0, "server không có mod nào trong mods/");

    controller.post(&format!("/servers/{id}/actions/stop"), json!({}));
    controller.wait_state(&id, "stopped", Duration::from_secs(300));
    wait_until("process Java thoát", Duration::from_secs(60), || {
        (!process_alive(pid)).then_some(())
    });
    assert!(controller.shutdown(Duration::from_secs(60)).success());
    out.status = "passed";
    out
}

fn wait_job(controller: &Controller, job: u64, pack: &Pack) -> Value {
    let limit = Duration::from_secs(pack.install_timeout_s);
    let state = wait_until("Job cài modpack xong", limit, || {
        let job = controller.get(&format!("/jobs/{job}"));
        (job["state"] != "running" && job["state"] != "queued").then_some(job)
    });
    assert_eq!(state["state"], "succeeded", "Job lỗi: {state}");
    state
}

fn count_jars(dir: &Path) -> u64 {
    std::fs::read_dir(dir).map_or(0, |entries| {
        entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "jar"))
            .count() as u64
    })
}

/// Đóng vai người dùng tải tay file CurseForge cấm phân phối: tải từ trang CurseForge một lần, giữ
/// trong `<data>/manual-files/` cho các đêm sau, rồi gửi qua API (API tự đối chiếu SHA-1).
fn supply_manual_files(controller: &Controller, data: &DataDir, id: &str) {
    let dir = data.path().join("manual-files");
    std::fs::create_dir_all(&dir).unwrap();
    let preflight = controller.get(&format!("/servers/{id}/preflight"));
    for item in preflight["pending"].as_array().cloned().unwrap_or_default() {
        let name = item["file_name"].as_str().unwrap().to_owned();
        let cached = dir.join(format!("{}-{name}", item["sha1"].as_str().unwrap()));
        if !cached.is_file() {
            let url = format!(
                "https://www.curseforge.com/api/v1/mods/{}/files/{}/download",
                item["project_id"], item["file_id"]
            );
            let ok = Command::new("curl")
                .args(["-fsSL", "--retry", "3", "-o"])
                .arg(&cached)
                .arg(&url)
                .status()
                .is_ok_and(|s| s.success());
            assert!(ok, "không tải được {name} từ {url}");
        }
        let status = upload_pending(controller, id, &name, &std::fs::read(&cached).unwrap());
        if !(200..300).contains(&status) {
            // File cache hỏng thì xóa để đêm sau tải lại.
            let _ = std::fs::remove_file(&cached);
            panic!("gửi {name} bị từ chối: HTTP {status}");
        }
    }
    let after = controller.get(&format!("/servers/{id}/preflight"));
    let left = &after["pending"];
    assert!(
        left.as_array().is_none_or(Vec::is_empty),
        "vẫn còn file chờ: {left}"
    );
}

/// `multipart/form-data` tối giản qua `TcpStream`, giống helper JSON trong `support`.
fn upload_pending(controller: &Controller, id: &str, name: &str, bytes: &[u8]) -> u16 {
    let host = controller.url.trim_start_matches("http://");
    let boundary = "minehost-modpack-test-boundary";
    let disposition = format!("form-data; name=\"file\"; filename=\"{name}\"");
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: {disposition}\r\nContent-Type: application/java-archive\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let path = format!("/api/v1/servers/{id}/content/pending/upload");
    let content_type = format!("multipart/form-data; boundary={boundary}");
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nX-MineHost: 1\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut stream = TcpStream::connect(host).unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(&body).unwrap();
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    String::from_utf8_lossy(&response)
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// RSS cao nhất của process (`VmHWM`), để chỉnh công thức RAM đề xuất (spec P3 mục 3.8).
fn peak_rss_mb(pid: u32) -> u64 {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
        .map_or(0, |kb| kb / 1024)
}

fn report(pack: &Pack, out: &Outcome) {
    let line = json!({
        "id": pack.id, "source": pack.source, "loader": pack.loader,
        "minecraft": pack.minecraft, "memory_mb": pack.memory_mb, "status": out.status,
        "install_s": out.install_s.round(), "done_s": out.done_s.round(),
        "peak_rss_mb": out.peak_rss_mb, "mods": out.mods, "skipped_client": out.skipped_client,
        "error": out.error, "note": pack.note,
    });
    eprintln!("{line}");
    if let Some(path) = std::env::var_os("MINEHOST_MODPACK_REPORT") {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(file, "{line}").unwrap();
    }
}

#[test]
#[ignore = "tải modpack thật, cần tới 10 GB RAM; chạy bằng workflow modpacks trên runner tự host"]
fn modpacks_reach_done() {
    let packs = load_packs();
    assert!(!packs.is_empty(), "không có pack nào để chạy");
    let mut failed = Vec::new();
    for pack in &packs {
        let out = catch_unwind(AssertUnwindSafe(|| run_pack(pack))).unwrap_or_else(|panic| {
            let msg = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_default();
            Outcome {
                status: "failed",
                error: msg,
                ..Outcome::default()
            }
        });
        report(pack, &out);
        if out.status == "failed" {
            failed.push(pack.id.as_str());
        }
    }
    assert!(failed.is_empty(), "pack hỏng: {failed:?}");
}
