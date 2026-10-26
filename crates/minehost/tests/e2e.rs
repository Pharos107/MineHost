//! Binary `minehost` với catalog giả: tạo Paper 1.21.4, bật, tắt. Không gọi mạng.
#![cfg(feature = "e2e")]
#![allow(clippy::unwrap_used)]

mod support;

use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;
use support::{Controller, DataDir, wait_until};

#[test]
fn paper_with_fake_java_reaches_running_then_stops() {
    let java = PathBuf::from(env!("CARGO_BIN_EXE_e2e-fake-mc"));
    let dir = DataDir::prepare();
    let controller = Controller::start_with_env(&dir, &[("MINEHOST_E2E_JAVA", java.as_path())]);

    let versions = controller.get("/catalog/paper/versions");
    let ids: Vec<&str> = versions
        .as_array()
        .unwrap()
        .iter()
        .map(|version| version["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["1.21.4", "1.20.4"]);

    let created = controller.post(
        "/servers",
        json!({
            "name": "E2E",
            "loader": "paper",
            "minecraft": "1.21.4",
            "memory_mb": { "min": 512, "max": 1024 }
        }),
    );
    let job = created["job_id"].as_u64().unwrap();
    let state = wait_until("Job tạo server xong", Duration::from_secs(30), || {
        let job = controller.get(&format!("/jobs/{job}"));
        (job["state"] != "running").then_some(job)
    });
    assert_eq!(state["state"], "succeeded", "Job lỗi: {state}");
    let id = created["server_id"].as_str().unwrap();

    controller.post(&format!("/servers/{id}/eula"), json!({ "accept": true }));
    controller.post(&format!("/servers/{id}/actions/start"), json!({}));
    controller.wait_state(id, "running", Duration::from_secs(30));
    controller.post(
        &format!("/servers/{id}/console"),
        json!({ "command": "list" }),
    );
    controller.wait_console(id, "There are 0 of a max of 20 players online:");
    controller.post(&format!("/servers/{id}/actions/stop"), json!({}));
    controller.wait_state(id, "stopped", Duration::from_secs(30));
    assert!(
        controller.log().contains("chế độ e2e"),
        "không thấy cảnh báo e2e trong log:\n{}",
        controller.log()
    );

    let status = controller.shutdown(Duration::from_secs(30));
    assert!(status.success(), "{status}");
}
