//! API key CurseForge (spec P3, mục 3.6 và 8).
//!
//! Kiểu bọc ẩn giá trị khỏi `Debug`, `Display`, lỗi và không cho `serde::Serialize` để key không
//! bao giờ lọt ra log hay response API.

use std::fmt;
use std::str::FromStr;

use crate::error::InvalidValue;

/// API key CurseForge đã bỏ khoảng trắng hai đầu và kiểm tra không rỗng, không ký tự điều khiển.
#[derive(Clone, PartialEq, Eq)]
pub struct CurseforgeKey(String);

impl CurseforgeKey {
    pub fn parse(raw: &str) -> Result<Self, InvalidValue> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(InvalidValue::new("CurseforgeKey", "rỗng"));
        }
        if trimmed.chars().any(char::is_control) {
            return Err(InvalidValue::new("CurseforgeKey", "có ký tự điều khiển"));
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Chỉ gọi khi gửi header tới CurseForge hoặc ghi `secrets.toml`.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for CurseforgeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl fmt::Display for CurseforgeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl FromStr for CurseforgeKey {
    type Err = InvalidValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for CurseforgeKey {
    type Error = InvalidValue;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_whitespace_and_control_chars() {
        for bad in ["", "   ", "\t\n", "abc\ndef", "abc\0def"] {
            let err = CurseforgeKey::parse(bad).unwrap_err();
            assert!(!err.to_string().contains("abc"));
        }
    }

    #[test]
    fn trims_and_masks_debug_display() {
        let raw = "$2a$10$secret-curseforge-key";
        let key = CurseforgeKey::parse(&format!("  {raw}  ")).unwrap();
        assert_eq!(key.expose(), raw);
        assert_eq!(format!("{key:?}"), "***");
        assert_eq!(format!("{key}"), "***");
        assert!(!format!("{key:?}").contains(raw));
        assert!(!format!("{key}").contains(raw));
    }
}
