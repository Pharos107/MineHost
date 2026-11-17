//! Test `ProcessRuntime` với `fake-mc`; không cần Java.

// Hàm trợ giúp ngoài `#[test]` không được clippy coi là code test.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::time::Duration;

use mh_core::ConsoleStream;
use mh_runtime::{OutputLine, ProcessRuntime, ProcessSpec, RunningProcess};

const WAIT: Duration = Duration::from_secs(10);

fn fake_mc(args: &[&str]) -> ProcessSpec {
    ProcessSpec {
        program: PathBuf::from(env!("CARGO_BIN_EXE_fake-mc")),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        work_dir: std::env::temp_dir(),
        limits: None,
    }
}

/// Đọc output tới khi gặp dòng chứa `needle`; trả mọi dòng đã đọc.
async fn read_until(proc: &mut RunningProcess, needle: &str) -> Vec<OutputLine> {
    let mut seen = Vec::new();
    let found = tokio::time::timeout(WAIT, async {
        while let Some(line) = proc.output.recv().await {
            let hit = line.text.contains(needle);
            seen.push(line);
            if hit {
                return true;
            }
        }
        false
    })
    .await;
    assert_eq!(found, Ok(true), "không thấy {needle:?}; đã đọc {seen:?}");
    seen
}

async fn exit_code(proc: RunningProcess) -> Option<i32> {
    tokio::time::timeout(WAIT, proc.exit)
        .await
        .unwrap()
        .unwrap()
        .code
}

#[tokio::test]
async fn vietnamese_survives_the_pipe() {
    let rt = ProcessRuntime::new().unwrap();
    let mut proc = rt.spawn(fake_mc(&[])).await.unwrap();

    let lines = read_until(&mut proc, "Done (").await;
    assert!(
        lines
            .iter()
            .any(|l| l.text == "[10:00:00 INFO]: Xin chào thế giới — tiếng Việt có dấu: ưỡẫ"),
        "{lines:?}"
    );

    proc.control
        .send_line("say Đã vào được server ạ")
        .await
        .unwrap();
    read_until(&mut proc, "[Server] Đã vào được server ạ").await;
}

/// Đúng dòng lệnh MineHost dùng để bật server: cờ JVM bị bỏ qua, `Done` / `list` / `stop` vẫn chạy.
#[tokio::test]
async fn java_flags_are_ignored_and_list_reports_nobody() {
    let rt = ProcessRuntime::new().unwrap();
    let mut proc = rt
        .spawn(fake_mc(&[
            "-Xms512M",
            "-Xmx1024M",
            "-Dfile.encoding=UTF-8",
            "-jar",
            "server.jar",
            "--nogui",
        ]))
        .await
        .unwrap();
    read_until(&mut proc, "Done (").await;
    proc.control.send_line("list").await.unwrap();
    read_until(&mut proc, "There are 0 of a max of 20 players online:").await;
    proc.control.send_line("stop").await.unwrap();
    read_until(&mut proc, "Stopping server").await;
    assert_eq!(exit_code(proc).await, Some(0));
}

#[tokio::test]
async fn stderr_is_merged_and_tagged() {
    let rt = ProcessRuntime::new().unwrap();
    let mut proc = rt.spawn(fake_mc(&[])).await.unwrap();
    // Hai pipe riêng nên thứ tự giữa stdout và stderr không cố định; chỉ kiểm tra đủ cả hai.
    let mut lines = Vec::new();
    while !(lines
        .iter()
        .any(|l: &OutputLine| l.text.contains("ra stderr"))
        && lines.iter().any(|l| l.text.contains("Done (")))
    {
        let line = tokio::time::timeout(WAIT, proc.output.recv())
            .await
            .unwrap()
            .unwrap();
        lines.push(line);
    }
    let tag = |needle: &str| {
        lines
            .iter()
            .find(|l| l.text.contains(needle))
            .unwrap()
            .stream
    };
    assert_eq!(tag("ra stderr"), ConsoleStream::Stderr);
    assert_eq!(tag("Done ("), ConsoleStream::Stdout);
    proc.control.kill();
}

#[tokio::test]
async fn stop_exits_cleanly_and_closes_output() {
    let rt = ProcessRuntime::new().unwrap();
    let mut proc = rt.spawn(fake_mc(&[])).await.unwrap();
    read_until(&mut proc, "Done (").await;

    proc.control.send_line("stop").await.unwrap();
    read_until(&mut proc, "Stopping server").await;
    let control = proc.control.clone();
    assert_eq!(exit_code(proc).await, Some(0));
    // Process đã thoát thì gửi lệnh báo lỗi chứ không treo.
    let sent = tokio::time::timeout(WAIT, async {
        loop {
            if control.send_line("say x").await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(sent.is_ok());
}

#[tokio::test]
async fn kill_ends_a_hung_process() {
    let rt = ProcessRuntime::new().unwrap();
    let mut proc = rt.spawn(fake_mc(&["--hang-on-stop"])).await.unwrap();
    read_until(&mut proc, "Done (").await;
    proc.control.send_line("stop").await.unwrap();
    read_until(&mut proc, "giả vờ treo").await;

    proc.control.kill();
    proc.control.kill();
    let code = exit_code(proc).await;
    assert_ne!(code, Some(0));
}

#[tokio::test]
async fn crash_reports_exit_code() {
    let rt = ProcessRuntime::new().unwrap();
    let mut proc = rt
        .spawn(fake_mc(&["--crash-after-ms", "50"]))
        .await
        .unwrap();
    read_until(&mut proc, "OutOfMemoryError").await;
    assert_eq!(exit_code(proc).await, Some(1));
}

#[tokio::test]
async fn multi_line_command_is_rejected() {
    let rt = ProcessRuntime::new().unwrap();
    let proc = rt.spawn(fake_mc(&[])).await.unwrap();
    let err = proc
        .control
        .send_line("say a\nop hacker")
        .await
        .unwrap_err();
    assert_eq!(err.code(), mh_core::ErrorCode::InvalidInput);
    proc.control.kill();
}

#[tokio::test]
async fn missing_program_is_a_spawn_error() {
    let rt = ProcessRuntime::new().unwrap();
    let mut spec = fake_mc(&[]);
    spec.program = PathBuf::from("does-not-exist-mh");
    let err = rt.spawn(spec).await.unwrap_err();
    assert!(
        matches!(err, mh_runtime::RuntimeError::Spawn { .. }),
        "{err:?}"
    );
}

/// Trên Linux, `PR_SET_PDEATHSIG` kill process con khi **luồng** đã fork thoát. Spawn từ một
/// luồng tạm (có runtime riêng) rồi để luồng đó thoát: server vẫn phải sống.
#[tokio::test(flavor = "multi_thread")]
async fn process_survives_when_calling_thread_exits() {
    let rt = std::sync::Arc::new(ProcessRuntime::new().unwrap());
    let rt2 = std::sync::Arc::clone(&rt);
    let mut proc = std::thread::spawn(move || {
        let local = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        local.block_on(rt2.spawn(fake_mc(&[]))).unwrap()
    })
    .join()
    .unwrap();

    read_until(&mut proc, "Done (").await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    proc.control.send_line("say vẫn sống").await.unwrap();
    read_until(&mut proc, "[Server] vẫn sống").await;
    proc.control.send_line("stop").await.unwrap();
    assert_eq!(exit_code(proc).await, Some(0));
}

#[test]
fn needs_a_tokio_runtime() {
    let err = ProcessRuntime::new().unwrap_err();
    assert!(matches!(err, mh_runtime::RuntimeError::NoTokio));
}

/// Luồng spawner dừng (controller thoát) thì OS kill process con: Job Object trên Windows,
/// `PR_SET_PDEATHSIG` trên Linux.
#[cfg(any(windows, target_os = "linux"))]
#[tokio::test]
async fn dropping_runtime_kills_children() {
    let rt = ProcessRuntime::new().unwrap();
    let mut proc = rt.spawn(fake_mc(&["--hang-on-stop"])).await.unwrap();
    read_until(&mut proc, "Done (").await;

    // `fake-mc` chỉ tự thoát khi stdin đóng, mà `proc.control` vẫn giữ stdin mở: nếu `exit`
    // tới thì process đã bị OS kill. Mã thoát không dùng được, vì Job Object kill với mã 0.
    drop(rt);
    exit_code(proc).await;
}

/// Server không đọc stdin: gửi lệnh báo đầy ngay thay vì chờ, và kill vẫn có tác dụng.
#[tokio::test]
async fn full_stdin_fails_fast_and_kill_still_works() {
    let rt = ProcessRuntime::new().unwrap();
    let mut proc = rt.spawn(fake_mc(&["--fail-start-hang"])).await.unwrap();
    read_until(&mut proc, "Failed to start").await;

    let command = format!("say {}", "x".repeat(4096));
    let full = tokio::time::timeout(WAIT, async {
        for _ in 0..10_000 {
            match proc.control.send_line(&command).await {
                Ok(()) => tokio::task::yield_now().await,
                Err(err) => return Some(err),
            }
        }
        None
    })
    .await
    .unwrap();
    assert!(
        matches!(full, Some(mh_runtime::RuntimeError::StdinFull)),
        "{full:?}"
    );
    proc.control.kill();
    exit_code(proc).await;
}

/// Process cháu (plugin chạy chương trình ngoài) chết theo khi server bị kill.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn kill_takes_down_grandchildren() {
    let rt = ProcessRuntime::new().unwrap();
    let mut proc = rt.spawn(fake_mc(&["--spawn-child"])).await.unwrap();
    let lines = read_until(&mut proc, "child pid ").await;
    let pid: u32 = lines
        .iter()
        .find_map(|l| l.text.split("child pid ").nth(1))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(alive(pid));
    proc.control.kill();
    exit_code(proc).await;
    // Process cháu mồ côi được init thu dọn; chờ một chút để nó biến khỏi `/proc` (hoặc chỉ còn zombie).
    let deadline = std::time::Instant::now() + WAIT;
    while alive(pid) {
        assert!(
            std::time::Instant::now() < deadline,
            "process cháu {pid} vẫn sống"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(target_os = "linux")]
fn alive(pid: u32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        // Trường thứ 3 là trạng thái; `Z` là đã chết, chờ được thu dọn.
        Ok(stat) => stat
            .rsplit_once(") ")
            .is_some_and(|(_, rest)| !rest.starts_with('Z')),
        Err(_) => false,
    }
}
