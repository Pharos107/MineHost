//! Server Minecraft giả cho test: in log giống Paper và đọc lệnh từ stdin. Không cần Java.
//!
//! Được gọi đúng như Java (`-Xms`, `-Xmx`, `-D…`, `-jar server.jar`, `--nogui`): những cờ đó
//! không có ý nghĩa ở đây và bị bỏ qua. Thư mục làm việc là thư mục server, giống process thật.
//!
//! Cờ:
//! - `--no-done`: không bao giờ in dòng `Done (…)!`.
//! - `--hang-on-stop`: bỏ qua lệnh `stop`, giống server bị treo khi lưu world.
//! - `--crash-after-ms <n>`: in lỗi hết RAM rồi thoát mã 1 sau `n` mili giây.
//! - `--slow-start-ms <n>`: chờ `n` mili giây trước khi in dòng `Done`.
//! - `--hang-on-save-all` (hoặc `MINEHOST_FAKE_SAVE_ALL_HANG=1`): `save-all flush` không in
//!   dòng `Saved the game`, để test thời hạn chờ khi sao lưu.
//!
//! Chế độ installer giả (spec P3, PR-02), khi có `--install`: in vài dòng, tạo các file
//! `--write <đường dẫn tương đối>` (lặp được), chờ `--sleep-ms <n>`, đọc stdin tới EOF nếu có
//! `--read-stdin`, rồi thoát với mã `--exit-code <n>` (mặc định 0). Không in dòng `Done`.
//!
//! Lệnh stdin: `stop` thoát, `say …` in lại nội dung; `save-off`, `save-all flush`, `save-on`
//! (sao lưu); cho test người chơi: `list`,
//! `fake-join <tên>`, `fake-leave <tên>`.

// Đây là chương trình dòng lệnh, in ra stdout/stderr là việc chính của nó.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::BufRead;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if has_flag(&args, "--install") {
        install(&args);
    }
    print_banner();
    schedule_crash(flag_number(&args, "--crash-after-ms"));
    if let Some(ms) = flag_number(&args, "--slow-start-ms") {
        std::thread::sleep(Duration::from_millis(ms));
    }
    if !has_flag(&args, "--no-done") {
        println!("[10:00:01 INFO]: Done (0.100s)! For help, type \"help\"");
    }
    let hang_save_all = has_flag(&args, "--hang-on-save-all")
        || std::env::var("MINEHOST_FAKE_SAVE_ALL_HANG").ok().as_deref() == Some("1");
    read_commands(has_flag(&args, "--hang-on-stop"), hang_save_all);
}

fn install(args: &[String]) -> ! {
    println!("Installing fake loader");
    for (i, arg) in args.iter().enumerate() {
        if arg != "--write" {
            continue;
        }
        let Some(rel) = args.get(i + 1) else {
            continue;
        };
        let path = std::path::Path::new(rel);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(path, b"-Dfake=1
") {
            Ok(()) => println!("Wrote {rel}"),
            Err(e) => eprintln!("Cannot write {rel}: {e}"),
        }
    }
    if let Some(ms) = flag_number(args, "--sleep-ms") {
        std::thread::sleep(Duration::from_millis(ms));
    }
    if has_flag(args, "--read-stdin") {
        let lines = std::io::stdin().lock().lines().count();
        println!("stdin closed after {lines} lines");
    }
    let code = flag_number(args, "--exit-code").unwrap_or(0);
    eprintln!("Installer finished with code {code}");
    std::process::exit(i32::try_from(code).unwrap_or(1));
}

fn print_banner() {
    println!("[10:00:00 INFO]: Starting minecraft server version fake");
    println!("[10:00:00 INFO]: Xin chào thế giới — tiếng Việt có dấu: ưỡẫ");
    eprintln!("[10:00:00 WARN]: dòng này ra stderr");
}

fn schedule_crash(after_ms: Option<u64>) {
    let Some(ms) = after_ms else {
        return;
    };
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ms));
        println!("java.lang.OutOfMemoryError: Java heap space");
        std::process::exit(1);
    });
}

fn read_commands(hang_on_stop: bool, hang_save_all: bool) {
    let mut online = Vec::new();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else {
            break;
        };
        let cmd = line.trim();
        if save_command(cmd, hang_save_all) {
            continue;
        }
        if !on_command(cmd, hang_on_stop, &mut online) {
            return;
        }
    }
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn flag_number(args: &[String], flag: &str) -> Option<u64> {
    let index = args.iter().position(|arg| arg == flag)?;
    args.get(index + 1)?.parse().ok()
}

/// `false` khi lệnh `stop` được nhận và process nên thoát.
fn on_command(cmd: &str, hang_on_stop: bool, online: &mut Vec<String>) -> bool {
    if cmd == "stop" {
        return on_stop(hang_on_stop);
    }
    if cmd == "list" {
        print_online(online);
        return true;
    }
    if let Some(name) = cmd.strip_prefix("fake-join ") {
        join(online, name);
        return true;
    }
    if let Some(name) = cmd.strip_prefix("fake-leave ") {
        leave(online, name);
        return true;
    }
    match cmd.strip_prefix("say ") {
        Some(text) => println!("[10:00:02 INFO]: [Server] {text}"),
        None => println!("[10:00:02 INFO]: Unknown command: {cmd}"),
    }
    true
}

fn on_stop(hang_on_stop: bool) -> bool {
    if hang_on_stop {
        println!("[10:00:02 INFO]: Stopping server (giả vờ treo)");
        return true;
    }
    println!("[10:00:02 INFO]: Stopping server");
    false
}

fn join(online: &mut Vec<String>, raw: &str) {
    let name = raw.trim();
    if name.is_empty() {
        return;
    }
    if !online.iter().any(|have| have.eq_ignore_ascii_case(name)) {
        online.push(name.to_owned());
    }
    println!("[10:00:03 INFO]: {name} joined the game");
}

fn leave(online: &mut Vec<String>, raw: &str) {
    let name = raw.trim();
    if name.is_empty() {
        return;
    }
    online.retain(|have| !have.eq_ignore_ascii_case(name));
    println!("[10:00:03 INFO]: {name} left the game");
}

fn print_online(online: &[String]) {
    let names = online.join(", ");
    let count = online.len();
    // Dấu cách sau dấu hai chấm có cả khi danh sách rỗng, đúng dòng Vanilla/Paper.
    println!("[10:00:03 INFO]: There are {count} of a max of 20 players online: {names}");
}

/// `true` khi `cmd` là lệnh lưu world. `hang` thì `save-all flush` không in dòng xong.
fn save_command(cmd: &str, hang: bool) -> bool {
    match cmd {
        "save-off" => println!("[10:00:02 INFO]: Automatic saving is now disabled"),
        "save-on" => println!("[10:00:02 INFO]: Automatic saving is now enabled"),
        "save-all flush" => {
            println!("[10:00:02 INFO]: Saving the game (this may take a moment!)");
            if !hang {
                println!("[10:00:02 INFO]: Saved the game");
            }
        }
        _ => return false,
    }
    true
}
