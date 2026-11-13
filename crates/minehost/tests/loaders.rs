//! Test tích hợp: cài thật từng mod loader (Fabric, Quilt, Forge, NeoForge),
//! bật tới khi thấy `Done` rồi tắt sạch sẽ. Mặc định bị `#[ignore]` vì tải
//! từ Internet; chạy trong job `integration` của CI trên cả Windows và Linux.

#![allow(clippy::expect_used, clippy::unwrap_used)]

mod support;

use std::time::Duration;

use serde_json::json;

use support::{Controller, DataDir, process_alive, wait_until};

fn check_loader_lifecycle(loader: &str) {
    let data = DataDir::prepare();
    let controller = Controller::start(&data);
    let id = controller.create_server_of(&format!("{loader}-server"), loader);

    let pid = controller.start_server(&id);
    assert!(process_alive(pid), "server {loader} phải đang sống");

    controller.post(&format!("/servers/{id}/actions/stop"), json!({}));
    controller.wait_state(&id, "stopped", Duration::from_secs(60));
    wait_until("process Java thoát", Duration::from_secs(15), || {
        (!process_alive(pid)).then_some(())
    });

    assert!(controller.shutdown(Duration::from_secs(20)).success());
}

#[test]
#[ignore = "tải Java và Fabric thật; chạy trong job integration của CI"]
fn fabric_reaches_done() {
    check_loader_lifecycle("fabric");
}

#[test]
#[ignore = "tải Java và Quilt thật; chạy trong job integration của CI"]
fn quilt_reaches_done() {
    check_loader_lifecycle("quilt");
}

#[test]
#[ignore = "tải Java và Forge thật; chạy trong job integration của CI"]
fn forge_reaches_done() {
    check_loader_lifecycle("forge");
}

#[test]
#[ignore = "tải Java và NeoForge thật; chạy trong job integration của CI"]
fn neoforge_reaches_done() {
    check_loader_lifecycle("neoforge");
}
