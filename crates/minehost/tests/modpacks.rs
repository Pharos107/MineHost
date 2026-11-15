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

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stderr)]

mod support;

use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
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
    out.install_s = started.elapsed().as_secs_f64();
    out.mods = job["result"]["installed_mods"].as_u64().unwrap_or(0);
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
    let pending = &state["result"]["pending_curseforge"];
    assert!(
        pending.as_array().is_none_or(Vec::is_empty),
        "còn file CurseForge chờ tải tay: {pending}"
    );
    state
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
