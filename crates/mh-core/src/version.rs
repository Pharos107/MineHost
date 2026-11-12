//! Phiên bản Minecraft và loại server.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::InvalidValue;

/// Phiên bản Minecraft, ví dụ `1.21.4`, `26.3`, `26.4-snapshot-5`.
///
/// Chỉ là chuỗi đã kiểm tra, không phân tích thành số: Mojang đổi cách đánh số nhiều lần,
/// nên thứ tự lấy từ `releaseTime` trong manifest (spec P1, mục 3.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct McVersion(String);

impl McVersion {
    const MAX_LEN: usize = 64;

    /// Cho phép chữ, số ASCII và `. - _ +`; khoảng trắng chỉ ở giữa, vì vài bản pre-release
    /// cũ của Mojang có tên kiểu `1.14 Pre-Release 1`.
    pub fn parse(s: &str) -> Result<Self, InvalidValue> {
        let err = |reason| Err(InvalidValue::new("McVersion", reason));
        if s.is_empty() || s.len() > Self::MAX_LEN {
            return err("rỗng hoặc dài quá 64 ký tự");
        }
        let starts_ok = s.starts_with(|c: char| c.is_ascii_alphanumeric());
        let ends_ok = s.ends_with(|c: char| c.is_ascii_alphanumeric());
        if !starts_ok || !ends_ok {
            return err("phải bắt đầu và kết thúc bằng chữ hoặc số");
        }
        let allowed =
            |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+' | ' ');
        if !s.chars().all(allowed) {
            return err("có ký tự không cho phép");
        }
        Ok(Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for McVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for McVersion {
    type Err = InvalidValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for McVersion {
    type Error = InvalidValue;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<McVersion> for String {
    fn from(v: McVersion) -> Self {
        v.0
    }
}

/// Loại server. P1 có Paper; P2 thêm Vanilla và Purpur (spec P2, mục 5.4); mod loader từ P3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LoaderKind {
    Paper,
    Vanilla,
    Purpur,
}

impl LoaderKind {
    /// Chuỗi giống hệt giá trị serde.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Paper => "paper",
            Self::Vanilla => "vanilla",
            Self::Purpur => "purpur",
        }
    }

    pub fn parse(s: &str) -> Result<Self, InvalidValue> {
        match s {
            "paper" => Ok(Self::Paper),
            "vanilla" => Ok(Self::Vanilla),
            "purpur" => Ok(Self::Purpur),
            _ => Err(InvalidValue::new("LoaderKind", "loại server chưa hỗ trợ")),
        }
    }
}

impl fmt::Display for LoaderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LoaderKind {
    type Err = InvalidValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mc_version_accepts_mojang_names() {
        let names = [
            "1.21.4",
            "26.3",
            "1.21.11",
            "26.4-snapshot-5",
            "24w14a",
            "1.14 Pre-Release 1",
            "b1.7.3",
        ];
        for ok in names {
            assert_eq!(McVersion::parse(ok).unwrap().as_str(), ok);
        }
    }

    #[test]
    fn mc_version_rejects_bad_strings() {
        let long = "1".repeat(65);
        let bad = [
            "",
            " 1.21",
            "1.21 ",
            "../1.21",
            "1.21/x",
            ".",
            "1.21\n",
            "phiên-bản",
            long.as_str(),
        ];
        for s in bad {
            assert!(McVersion::parse(s).is_err(), "{s:?}");
        }
        assert!(serde_json::from_str::<McVersion>("\"..\"").is_err());
    }

    #[test]
    fn loader_kind_matches_serde_name() {
        for name in ["paper", "vanilla", "purpur"] {
            let kind = LoaderKind::parse(name).unwrap();
            assert_eq!(serde_json::to_string(&kind).unwrap(), format!("\"{name}\""));
            assert_eq!(kind.as_str(), name);
            assert_eq!(
                serde_json::from_str::<LoaderKind>(&format!("\"{name}\"")).unwrap(),
                kind
            );
        }
        assert!(LoaderKind::parse("Paper").is_err());
        assert!(LoaderKind::parse("forge").is_err());
        assert!(serde_json::from_str::<LoaderKind>("\"fabric\"").is_err());
    }
}
