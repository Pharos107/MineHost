//! Chạy thử `java -version` để chắc bản Java dùng được.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

/// Nhận đường dẫn `java`, trả chuỗi phiên bản hoặc lý do lỗi. Chạy trong `spawn_blocking`.
pub type ProbeFn = Arc<dyn Fn(&Path) -> Result<String, String> + Send + Sync>;

/// Không mở cửa sổ console khi controller chạy ẩn trên Windows.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn java_version(java_bin: &Path) -> Result<String, String> {
    let mut cmd = Command::new(java_bin);
    cmd.arg("-version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("không chạy được {}: {e}", java_bin.display()))?;
    if !out.status.success() {
        return Err(format!("java -version thoát với {}", out.status));
    }
    // `java -version` in ra stderr, không phải stdout.
    let text = String::from_utf8_lossy(&out.stderr);
    Ok(parse_version(&text).unwrap_or_else(|| text.lines().next().unwrap_or_default().to_owned()))
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
}
