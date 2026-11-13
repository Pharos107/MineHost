//! Bảng của `instance.toml` thêm ở P3 (spec P3, mục 4.1): cách chạy và nguồn modpack.

use std::fmt;

use serde::{Deserialize, Serialize};
use toml::Table;

use crate::error::InvalidValue;
use crate::launch::ArgsFilePath;

/// `[game.launch]`. Thiếu bảng này nghĩa là chạy bằng `-jar` như P1, P2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawLaunch", into = "RawLaunch")]
pub struct LaunchSection {
    pub mode: LaunchMode,
    pub extra: Table,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchMode {
    /// Chạy `game.jar` bằng `-jar`.
    Jar,
    ArgsFile(ArgsFilePath),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LaunchKindName {
    Jar,
    ArgsFile,
}

/// Dạng trên đĩa. Tách khỏi `LaunchSection` để `kind = "args_file"` thiếu `args_file` bị từ
/// chối ngay khi đọc, thay vì lỗi lúc bật server.
#[derive(Serialize, Deserialize)]
struct RawLaunch {
    kind: LaunchKindName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    args_file: Option<ArgsFilePath>,
    #[serde(flatten)]
    extra: Table,
}

impl TryFrom<RawLaunch> for LaunchSection {
    type Error = InvalidValue;

    fn try_from(raw: RawLaunch) -> Result<Self, Self::Error> {
        let mode = match (raw.kind, raw.args_file) {
            (LaunchKindName::Jar, None) => LaunchMode::Jar,
            (LaunchKindName::ArgsFile, Some(path)) => LaunchMode::ArgsFile(path),
            (LaunchKindName::Jar, Some(_)) => {
                return Err(InvalidValue::new("LaunchSection", "`jar` không có `args_file`"));
            }
            (LaunchKindName::ArgsFile, None) => {
                return Err(InvalidValue::new("LaunchSection", "thiếu `args_file`"));
            }
        };
        Ok(Self {
            mode,
            extra: raw.extra,
        })
    }
}

impl From<LaunchSection> for RawLaunch {
    fn from(section: LaunchSection) -> Self {
        let (kind, args_file) = match section.mode {
            LaunchMode::Jar => (LaunchKindName::Jar, None),
            LaunchMode::ArgsFile(path) => (LaunchKindName::ArgsFile, Some(path)),
        };
        Self {
            kind,
            args_file,
            extra: section.extra,
        }
    }
}

impl LaunchSection {
    pub fn new(mode: LaunchMode) -> Self {
        Self {
            mode,
            extra: Table::new(),
        }
    }
}

/// `[modpack]`: chỉ có khi server được tạo từ modpack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModpackSection {
    pub source: ModpackSource,
    /// Thiếu khi `source = "file"` (pack người dùng tải lên).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ContentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<ContentId>,
    pub name: UpstreamText,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_name: Option<UpstreamText>,
    #[serde(flatten)]
    pub extra: Table,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModpackSource {
    Modrinth,
    Curseforge,
    File,
}

/// Id của project hoặc phiên bản trên Modrinth (`AANobbMI`) hoặc CurseForge (`238222`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentId(String);

impl ContentId {
    pub fn parse(s: &str) -> Result<Self, InvalidValue> {
        let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_');
        if s.is_empty() || s.len() > 64 || !s.chars().all(allowed) {
            return Err(InvalidValue::new("ContentId", "chỉ được có chữ, số, - và _"));
        }
        Ok(Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Chữ lấy từ nguồn ngoài (tên pack, tên phiên bản): giới hạn độ dài, không có ký tự điều khiển.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct UpstreamText(String);

impl UpstreamText {
    const MAX_CHARS: usize = 200;

    pub fn parse(s: &str) -> Result<Self, InvalidValue> {
        let trimmed = s.trim();
        if trimmed.is_empty() || trimmed.chars().count() > Self::MAX_CHARS {
            return Err(InvalidValue::new("UpstreamText", "rỗng hoặc dài quá 200 ký tự"));
        }
        if trimmed.chars().any(char::is_control) {
            return Err(InvalidValue::new("UpstreamText", "có ký tự điều khiển"));
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

macro_rules! string_newtype {
    ($ty:ident) => {
        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl TryFrom<String> for $ty {
            type Error = InvalidValue;

            fn try_from(s: String) -> Result<Self, Self::Error> {
                Self::parse(&s)
            }
        }

        impl From<$ty> for String {
            fn from(v: $ty) -> Self {
                v.0
            }
        }
    };
}

string_newtype!(ContentId);
string_newtype!(UpstreamText);
