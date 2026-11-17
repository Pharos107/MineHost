//! Chạy thử `java -version` để chắc bản Java dùng được.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Nhận đường dẫn `java`, trả chuỗi phiên bản hoặc lý do lỗi. Chạy trong `spawn_blocking`.
pub type ProbeFn = Arc<dyn Fn(&Path) -> Result<String, String> + Send + Sync>;

/// Không mở cửa sổ console khi controller chạy ẩn trên Windows.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `java -version` thường xong dưới một giây; máy chậm, antivirus quét lần đầu vẫn dư sức. Treo quá
/// hạn này thì kill, để khóa theo `major` không bị giữ mãi.
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_EVERY: Duration = Duration::from_millis(50);

pub fn java_version(java_bin: &Path) -> Result<String, String> {
    java_version_within(java_bin, PROBE_TIMEOUT)
}

fn java_version_within(java_bin: &Path, timeout: Duration) -> Result<String, String> {
    let mut cmd = Command::new(java_bin);
    cmd.arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("không chạy được {}: {e}", java_bin.display()))?;
    let status = wait_within(&mut child, timeout)?;
    // `java -version` in ra stderr, không phải stdout. Vài trăm byte, không làm đầy pipe trước khi thoát.
    let mut raw = Vec::new();
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_end(&mut raw);
    }
    if !status.success() {
        return Err(format!("java -version thoát với {status}"));
    }
    let text = String::from_utf8_lossy(&raw);
    Ok(parse_version(&text).unwrap_or_else(|| text.lines().next().unwrap_or_default().to_owned()))
}

fn wait_within(child: &mut Child, timeout: Duration) -> Result<ExitStatus, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(e) => return Err(format!("không chờ được java -version: {e}")),
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("java -version chưa xong sau {timeout:?}"));
        }
        std::thread::sleep(POLL_EVERY);
    }
}

/// Lấy phần trong ngoặc kép ở dòng đầu: `openjdk version "21.0.5" 2024-10-15 LTS`.
fn parse_version(text: &str) -> Option<String> {
    let line = text.lines().next()?;
    let (_, rest) = line.split_once('"')?;
    let (version, _) = rest.split_once('"')?;
    Some(version.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openjdk_banner() {
        let text = "openjdk version \"21.0.5\" 2024-10-15 LTS\nOpenJDK Runtime Environment Temurin";
        assert_eq!(parse_version(text).as_deref(), Some("21.0.5"));
        assert_eq!(parse_version("weird output"), None);
    }

    #[test]
    fn missing_binary_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(java_version(&tmp.path().join("nope")).is_err());
    }

    /// Script treo thay cho `java`: quá hạn thì bị kill và trả lỗi, không chờ mãi.
    #[cfg(unix)]
    #[test]
    fn hanging_java_times_out() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = fake_java(tmp.path(), "sleep 30");
        let started = Instant::now();
        let err = java_version_within(&bin, Duration::from_millis(300)).unwrap_err();
        assert!(err.contains("chưa xong"), "{err}");
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[cfg(unix)]
    #[test]
    fn reads_version_from_stderr() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = fake_java(
            tmp.path(),
            "echo 'openjdk version \"21.0.5\" 2024-10-15 LTS' >&2",
        );
        assert_eq!(java_version(&bin).unwrap(), "21.0.5");
    }

    #[cfg(unix)]
    fn fake_java(dir: &Path, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let bin = dir.join("java");
        std::fs::write(&bin, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        bin
    }
}
