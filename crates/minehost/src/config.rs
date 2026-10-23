use std::net::SocketAddr;
use std::path::Path;

use anyhow::Context;
use mh_core::ErrorCode;
use serde::{Deserialize, Serialize};

/// Nội dung `config.toml` trong thư mục dữ liệu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Địa chỉ của giao diện web. Mặc định chỉ máy này truy cập được.
    pub listen: SocketAddr,
    /// Tự mở trình duyệt khi khởi động.
    pub open_browser: bool,
    /// Mức log, theo cú pháp `RUST_LOG`, ví dụ `info` hoặc `minehost=debug`.
    pub log_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: SocketAddr::from(([127, 0, 0, 1], 7878)),
            open_browser: true,
            log_level: "info".to_owned(),
        }
    }
}

/// Đọc `config.toml`; nếu chưa có thì tạo file với giá trị mặc định.
pub fn load_or_create(path: &Path) -> anyhow::Result<Config> {
    if !path.exists() {
        let config = Config::default();
        let text = toml::to_string_pretty(&config).context("không ghi được cấu hình mặc định")?;
        mh_platform::fs::write_atomic(path, text.as_bytes())?;
        return Ok(config);
    }

    let text = std::fs::read_to_string(path)
        .with_context(|| format!("không đọc được {}", path.display()))?;
    toml::from_str(&text).with_context(|| {
        format!(
            "[{}] file cấu hình {} không hợp lệ",
            ErrorCode::ConfigInvalid,
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_default_file_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");

        let config = load_or_create(&path).unwrap();
        assert_eq!(config, Config::default());
        assert_eq!(load_or_create(&path).unwrap(), config);
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "open_browser = false\n").unwrap();

        let config = load_or_create(&path).unwrap();
        assert!(!config.open_browser);
        assert_eq!(config.listen, Config::default().listen);
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "lisen = \"0.0.0.0:80\"\n").unwrap();

        let err = load_or_create(&path).unwrap_err();
        assert!(format!("{err:#}").contains("CONFIG_INVALID"));
    }
}
