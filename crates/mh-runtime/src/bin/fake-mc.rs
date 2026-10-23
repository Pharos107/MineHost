//! Server Minecraft giả cho test: in log giống Paper và đọc lệnh từ stdin. Không cần Java.
//!
//! Cờ:
//! - `--no-done`: không bao giờ in dòng `Done (…)!`.
//! - `--hang-on-stop`: bỏ qua lệnh `stop`, giống server bị treo khi lưu world.
//! - `--crash-after-ms <n>`: in lỗi hết RAM rồi thoát mã 1 sau `n` mili giây.
//! - `--slow-start-ms <n>`: chờ `n` mili giây trước khi in dòng `Done`.

// Đây là chương trình dòng lệnh, in ra stdout/stderr là việc chính của nó.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::BufRead;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
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
                println!("[10:00:02 INFO]: Stopping server (giả vờ treo)");
                continue;
            }
            println!("[10:00:02 INFO]: Stopping server");
            return;
        }
        match cmd.strip_prefix("say ") {
            Some(text) => println!("[10:00:02 INFO]: [Server] {text}"),
            None => println!("[10:00:02 INFO]: Unknown command: {cmd}"),
        }
    }
}
