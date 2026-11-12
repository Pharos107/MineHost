//! Test tích hợp với Paper và Java thật (spec P1, mục 1 "Tiêu chí hoàn thành" và mục 9).
//!
//! Cần mạng và vài phút, nên đánh dấu `#[ignore]`. CI chạy trong job `integration`:
//! `cargo test -p minehost --test paper -- --ignored --test-threads=1`.
//! Test tự đồng ý EULA cho server tạo trong thư mục tạm, đúng như luồng spec mô tả.

#![allow(clippy::unwrap_used)]

mod support;

use std::time::Duration;

use serde_json::json;
use support::{Controller, DataDir, process_alive, wait_until};

/// Tạo → EULA → bật → `Done` → gửi `list` → tắt → không còn Java.
#[test]
#[ignore = "tải Java và Paper thật; chạy trong job integration của CI"]
fn paper_full_flow() {
    let dir = DataDir::prepare();
    let controller = Controller::start(&dir);

    let id = controller.create_server("Tích hợp");
    let pid = controller.start_server(&id);
    assert!(process_alive(pid));

    controller.post(
        &format!("/servers/{id}/console"),
        json!({ "command": "list" }),
    );
    controller.wait_console(&id, "players online");

    controller.post(&format!("/servers/{id}/actions/stop"), json!({}));
    controller.wait_state(&id, "stopped", Duration::from_secs(90));
    wait_until("Java thoát", Duration::from_secs(10), || {
        (!process_alive(pid)).then_some(())
    });
    assert!(
        !controller.log().contains("buộc dừng"),
        "server phải dừng bằng `stop`"
    );

    let status = controller.shutdown(Duration::from_secs(30));
    assert!(status.success(), "{status}");
}

/// Kill controller thì Java chết theo trong 5 giây; chạy lại thì server tự bật lại.
#[test]
#[ignore = "tải Java và Paper thật; chạy trong job integration của CI"]
fn killed_controller_leaves_no_orphan_java() {
    let dir = DataDir::prepare();
    let controller = Controller::start(&dir);
    let id = controller.create_server("Mồ côi");
    let pid = controller.start_server(&id);

    controller.kill();
    wait_until(
        "Java chết theo controller",
        Duration::from_secs(5),
        || (!process_alive(pid)).then_some(()),
    );

    // Dấu `.running` còn lại nên lần chạy sau bật lại server.
    let controller = Controller::start(&dir);
    let server = controller.wait_state(&id, "running", Duration::from_secs(300));
    assert_ne!(server["pid"].as_u64(), Some(u64::from(pid)));
    let status = controller.shutdown(Duration::from_secs(90));
    assert!(status.success(), "{status}");
}

/// Bấm Thoát khi có 3 server chạy: cả 3 dừng bằng `stop`, không server nào bị kill,
/// world vẫn còn để mở lại.
#[test]
#[ignore = "tải Java và Paper thật; chạy trong job integration của CI"]
fn quit_stops_three_servers_cleanly() {
    let dir = DataDir::prepare();
    let controller = Controller::start(&dir);
    let ids: Vec<String> = (1..=3)
        .map(|n| controller.create_server(&format!("Server {n}")))
        .collect();
    let pids: Vec<u32> = ids.iter().map(|id| controller.start_server(id)).collect();

    let status = controller.shutdown(Duration::from_secs(180));
    assert!(status.success(), "{status}");
    for pid in pids {
        assert!(!process_alive(pid), "Java {pid} vẫn còn chạy");
    }

    let log = std::fs::read_dir(dir.path().join("logs"))
        .unwrap()
        .flatten()
        .map(|entry| std::fs::read_to_string(entry.path()).unwrap_or_default())
        .collect::<String>();
    assert!(
        !log.contains("buộc dừng"),
        "có server bị kill thay vì dừng bằng `stop`"
    );
    for id in &ids {
        let world = dir
            .path()
            .join("servers")
            .join(id)
            .join("server/world/level.dat");
        assert!(world.exists(), "thiếu {}", world.display());
    }
}

/// Vanilla và Purpur (spec P2, kế hoạch PR-30): tạo bản 1.21.4, bật tới `Done`, tắt bằng `stop`.
#[test]
#[ignore = "tải Java, jar Mojang và Purpur thật; chạy trong job integration của CI"]
fn vanilla_and_purpur_reach_done() {
    let dir = DataDir::prepare();
    let controller = Controller::start(&dir);
    for loader in ["vanilla", "purpur"] {
        let id = controller.create_server_of(&format!("Tích hợp {loader}"), loader);
        let pid = controller.start_server(&id);
        controller.post(&format!("/servers/{id}/actions/stop"), json!({}));
        controller.wait_state(&id, "stopped", Duration::from_secs(90));
        wait_until("Java thoát", Duration::from_secs(10), || {
            (!process_alive(pid)).then_some(())
        });
    }
    let status = controller.shutdown(Duration::from_secs(30));
    assert!(status.success(), "{status}");
}
