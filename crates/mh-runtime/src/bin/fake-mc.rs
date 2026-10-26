//! Server Minecraft giả cho test: in log giống Paper và đọc lệnh từ stdin. Không cần Java.
//!
//! Cờ:
//! - `--no-done`: không bao giờ in dòng `Done (…)!`.
//! - `--hang-on-stop`: bỏ qua lệnh `stop`, giống server bị treo khi lưu world.
//! - `--hang-on-save-all`: nhận `save-all flush` nhưng không in `Saved the game`.
//!   Biến `MINEHOST_FAKE_SAVE_ALL_HANG=1` cùng nghĩa, khi test không sửa được tham số Java.
//! - `--crash-after-ms <n>`: in lỗi hết RAM rồi thoát mã 1 sau `n` mili giây.
//! - `--slow-start-ms <n>`: chờ `n` mili giây trước khi in dòng `Done`.

// Đây là chương trình dòng lệnh, in ra stdout/stderr là việc chính của nó.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::{BufRead, Write};
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    let hang_save_all = has("--hang-on-save-all") || env_flag("MINEHOST_FAKE_SAVE_ALL_HANG");
    let number = |flag: &str| {
        let i = args.iter().position(|a| a == flag)?;
        args.get(i + 1)?.parse::<u64>().ok()
    };

    println!("[10:00:00 INFO]: Starting minecraft server version fake");
    println!("[10:00:00 INFO]: Xin chào thế giới — tiếng Việt có dấu: ưỡẫ");
    eprintln!("[10:00:00 WARN]: dòng này ra stderr");

    if let Some(ms) = number("--crash-after-ms") {
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(ms));
            println!("java.lang.OutOfMemoryError: Java heap space");
            std::process::exit(1);
        });
    }
    if let Some(ms) = number("--slow-start-ms") {
        std::thread::sleep(Duration::from_millis(ms));
    }
    if !has("--no-done") {
        println!("[10:00:01 INFO]: Done (0.100s)! For help, type \"help\"");
    }

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let cmd = line.trim();
        if cmd == "stop" {
            if has("--hang-on-stop") {
                emit("Stopping server (giả vờ treo)");
                continue;
            }
            emit("Stopping server");
            return;
        }
        if save_command(cmd, hang_save_all) {
            continue;
        }
        match cmd.strip_prefix("say ") {
            Some(text) => emit(&format!("[Server] {text}")),
            None => emit(&format!("Unknown command: {cmd}")),
        }
    }
}

/// `true` khi `cmd` là lệnh save. `hang` thì `save-all flush` không in dòng xong.
fn save_command(cmd: &str, hang: bool) -> bool {
    match cmd {
        "save-off" => emit("Automatic saving is now disabled"),
        "save-on" => emit("Automatic saving is now enabled"),
        "save-all flush" => {
            emit("Saving the game (this may take a moment!)");
            if !hang {
                emit("Saved the game");
            }
        }
        _ => return false,
    }
    true
}

/// In một dòng log kiểu Paper và đẩy ra ngay. Pipe không phải terminal thì stdout bị
/// đệm khối; không flush thì test chờ `Saved the game` mãi dù process đã in.
fn emit(message: &str) {
    println!("[10:00:02 INFO]: {message}");
    let _ = std::io::stdout().flush();
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).ok().as_deref() == Some("1")
}
