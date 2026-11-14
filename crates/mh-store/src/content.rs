//! Danh sách mod/plugin cài đặt (`servers/<id>/content.toml`) và bộ quét thư mục jar.
//!
//! `content.toml` nằm cạnh `instance.toml` (không trong `server/`), ghi nguồn gốc để cập nhật
//! và các file CurseForge còn chờ tải tay (spec P3, mục 4.2). Đĩa vẫn là nguồn dữ liệu chính:
//! bản ghi không còn file trên đĩa sẽ bị bỏ khi quét (mục 3.3).

mod scan;

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use toml::Table;

use crate::StoreError;

pub use scan::{ContentFile, ContentMetaError, ContentScanner, scan_content};

pub const CONTENT_FILE: &str = "content.toml";
pub const CONTENT_SCHEMA: u32 = 1;

/// Nguồn cài đặt của một file trong `content.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentSource {
    Modrinth,
    Curseforge,
    Modpack,
    Upload,
}

/// ID project trên nguồn ngoài: chuỗi trên Modrinth (`"AANobbMI"`), số trên CurseForge (`238222`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProjectId {
    Int(u64),
    Str(String),
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(n) => write!(f, "{n}"),
            Self::Str(s) => f.write_str(s),
        }
    }
}

/// Một mục `[[file]]` trong `content.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentRecord {
    /// Đường dẫn tương đối với `server/` theo dạng đang bật (`mods/x.jar`), kể cả khi nằm ở
    /// thư mục tắt.
    pub path: String,
    pub sha1: String,
    pub source: ContentSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ProjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    #[serde(default)]
    pub client_only: bool,
    #[serde(default)]
    pub from_modpack: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub installed_at: OffsetDateTime,
    #[serde(flatten)]
    pub extra: Table,
}

/// Một mục `[[pending]]` trong `content.toml` cho file CurseForge cấm phân phối qua API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingContent {
    pub path: String,
    pub project_id: u64,
    pub file_id: u64,
    pub sha1: String,
    pub page_url: String,
    #[serde(flatten)]
    pub extra: Table,
}

/// Nội dung `servers/<id>/content.toml`. Giữ khóa lạ ở gốc và trong từng mục.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentManifest {
    #[serde(default = "default_schema")]
    pub schema: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file: Vec<ContentRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending: Vec<PendingContent>,
    #[serde(flatten)]
    pub extra: Table,
}

const fn default_schema() -> u32 {
    CONTENT_SCHEMA
}

impl Default for ContentManifest {
    fn default() -> Self {
        Self {
            schema: CONTENT_SCHEMA,
            file: Vec::new(),
            pending: Vec::new(),
            extra: Table::new(),
        }
    }
}

#[derive(Deserialize)]
struct SchemaProbe {
    schema: Option<i64>,
}

impl ContentManifest {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        if text.trim().is_empty() {
            return Ok(Self::default());
        }
        let probe: SchemaProbe = toml::from_str(text).map_err(|e| e.to_string())?;
        if let Some(found) = probe.schema
            && found != i64::from(CONTENT_SCHEMA)
        {
            return Err(format!("schema {found}, chỉ hỗ trợ {CONTENT_SCHEMA}"));
        }
        toml::from_str(text).map_err(|e| e.to_string())
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }

    /// Đọc `content.toml`; file chưa tồn tại hoặc chỉ có khoảng trắng được coi là rỗng.
    pub fn load(path: &Path) -> Result<Self, StoreError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(StoreError::io(path, e)),
        };
        Self::from_toml(&text).map_err(|detail| StoreError::invalid_content(path, detail))
    }

    /// Ghi `content.toml` theo kiểu nguyên tử.
    pub fn save(&self, path: &Path) -> Result<(), StoreError> {
        let text = self
            .to_toml()
            .map_err(|e| StoreError::invalid_content(path, e))?;
        mh_platform::fs::write_atomic(path, text.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
