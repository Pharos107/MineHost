//! Đọc/ghi `<data>/secrets.toml` chứa API key bí mật của người dùng (spec P3, mục 4.3).
//!
//! File luôn được ghi bằng [`mh_platform::fs::write_atomic_private`] để đặt quyền `0600` trên Unix
//! cho file tạm trước khi ghi nội dung rồi mới đổi tên.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use mh_core::CurseforgeKey;
use serde::{Deserialize, Serialize};
use toml::Table;

use crate::StoreError;

pub const SECRETS_FILE: &str = "secrets.toml";

/// Nội dung `<data>/secrets.toml`. Giữ lại các khóa lạ để tương thích bản mới hơn.
#[derive(Clone, Default, PartialEq)]
pub struct Secrets {
    pub curseforge_api_key: Option<CurseforgeKey>,
    pub extra: Table,
}

impl fmt::Debug for Secrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Không in giá trị key hay khóa lạ vì `Debug` có thể đi vào log.
        f.debug_struct("Secrets")
            .field("has_curseforge_api_key", &self.curseforge_api_key.is_some())
            .field("extra_keys", &self.extra.len())
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
struct RawSecrets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    curseforge_api_key: Option<String>,
    #[serde(flatten)]
    extra: Table,
}

impl Secrets {
    pub fn from_toml(text: &str) -> Result<Self, &'static str> {
        if text.trim().is_empty() {
            return Ok(Self::default());
        }
        // Không đính kèm chuỗi lỗi của `toml` vì nó hay in trích đoạn dòng chứa key.
        let raw: RawSecrets =
            toml::from_str(text).map_err(|_| "secrets.toml không phải TOML hợp lệ")?;
        let curseforge_api_key = raw
            .curseforge_api_key
            .map(|value| CurseforgeKey::parse(&value))
            .transpose()
            .map_err(|_| "curseforge_api_key không hợp lệ")?;
        Ok(Self {
            curseforge_api_key,
            extra: raw.extra,
        })
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        let raw = RawSecrets {
            curseforge_api_key: self
                .curseforge_api_key
                .as_ref()
                .map(|key| key.expose().to_owned()),
            extra: self.extra.clone(),
        };
        toml::to_string(&raw)
    }

    /// Đọc `secrets.toml`; file chưa tồn tại hoặc chỉ có khoảng trắng được coi là trống.
    pub fn load(path: &Path) -> Result<Self, StoreError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(StoreError::io(path, e)),
        };
        Self::from_toml(&text).map_err(|detail| StoreError::invalid_secrets(path, detail))
    }

    /// Ghi `secrets.toml` theo kiểu nguyên tử với quyền `0600` trên Unix.
    ///
    /// Nếu không còn key và không còn khóa lạ nào thì xóa file để không để lại file rỗng.
    pub fn save(&self, path: &Path) -> Result<(), StoreError> {
        if self.curseforge_api_key.is_none() && self.extra.is_empty() {
            return match fs::remove_file(path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(StoreError::io(path, e)),
                _ => Ok(()),
            };
        }
        let text = self.to_toml().map_err(|_| {
            StoreError::invalid_secrets(path, "không tuần tự hóa được secrets.toml")
        })?;
        mh_platform::fs::write_atomic_private(path, text.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use mh_core::ErrorCode;

    use super::*;

    #[test]
    fn missing_and_empty_secrets_load_as_default() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(SECRETS_FILE);
        assert_eq!(Secrets::load(&path).unwrap(), Secrets::default());

        fs::write(&path, "  \n\t ").unwrap();
        assert_eq!(Secrets::load(&path).unwrap(), Secrets::default());
    }

    #[test]
    fn round_trips_key_and_preserves_unknown_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(SECRETS_FILE);
        let raw = r#"
curseforge_api_key = "my-secret-cf-key"
future_token = "keep-this"

[future_section]
enabled = true
"#;
        fs::write(&path, raw).unwrap();

        let mut loaded = Secrets::load(&path).unwrap();
        assert_eq!(
            loaded
                .curseforge_api_key
                .as_ref()
                .map(CurseforgeKey::expose),
            Some("my-secret-cf-key")
        );
        assert_eq!(
            loaded.extra.get("future_token"),
            Some(&toml::Value::String("keep-this".into()))
        );
        assert!(loaded.extra.contains_key("future_section"));
        assert!(!format!("{loaded:?}").contains("my-secret-cf-key"));
        assert!(!format!("{loaded:?}").contains("keep-this"));

        loaded.curseforge_api_key = None;
        loaded.save(&path).unwrap();
        assert!(path.exists());

        let reloaded = Secrets::load(&path).unwrap();
        assert_eq!(reloaded.curseforge_api_key, None);
        assert_eq!(
            reloaded.extra.get("future_token"),
            Some(&toml::Value::String("keep-this".into()))
        );
        assert!(reloaded.extra.contains_key("future_section"));
    }

    #[test]
    fn clearing_without_unknown_keys_removes_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(SECRETS_FILE);
        let secrets = Secrets {
            curseforge_api_key: Some(CurseforgeKey::parse("user-key-1").unwrap()),
            extra: Table::new(),
        };
        secrets.save(&path).unwrap();
        assert!(path.exists());

        Secrets::default().save(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn invalid_toml_or_empty_key_errors_without_leaking_value() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(SECRETS_FILE);

        fs::write(&path, "curseforge_api_key = \"secret-broken\"\n@@@").unwrap();
        let err = Secrets::load(&path).unwrap_err();
        assert_eq!(err.code(), ErrorCode::ConfigInvalid);
        assert!(!err.to_string().contains("secret-broken"));

        fs::write(&path, "curseforge_api_key = \"   \"\n").unwrap();
        let err = Secrets::load(&path).unwrap_err();
        assert_eq!(err.code(), ErrorCode::ConfigInvalid);
    }

    #[cfg(unix)]
    #[test]
    fn save_enforces_mode_0600() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(SECRETS_FILE);
        fs::write(&path, "other = 1\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        let secrets = Secrets {
            curseforge_api_key: Some(CurseforgeKey::parse("unix-secret").unwrap()),
            extra: Table::new(),
        };
        secrets.save(&path).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
