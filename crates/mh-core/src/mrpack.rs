//! Định dạng index của modpack Modrinth (`.mrpack` / `modrinth.index.json`).
//!
//! Crate này không làm I/O, nên chỉ định nghĩa kiểu dữ liệu serde và các hàm thuần kiểm tra
//! đường dẫn, phiên bản định dạng, game và suy ra loader từ `dependencies`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::version::{LoaderKind, LoaderVersion, McVersion};

pub const MRPACK_FORMAT_VERSION: u32 = 1;
pub const MRPACK_GAME: &str = "minecraft";

/// Lỗi khi phân tích hoặc kiểm tra `modrinth.index.json`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MrpackError {
    #[error("modrinth.index.json không hợp lệ: {0}")]
    Json(String),
    #[error("formatVersion {0} không được hỗ trợ (chỉ nhận 1)")]
    UnsupportedFormat(u32),
    #[error("game `{0}` không được hỗ trợ (chỉ nhận minecraft)")]
    UnsupportedGame(String),
    #[error("đường dẫn `{path}` trong modpack không hợp lệ: {reason}")]
    InvalidPath { path: String, reason: &'static str },
    #[error("dependencies trong modpack không hợp lệ: {0}")]
    InvalidDependencies(&'static str),
}

/// Nội dung file `modrinth.index.json` ở gốc gói `.mrpack`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MrpackIndex {
    pub format_version: u32,
    pub game: String,
    pub version_id: String,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub files: Vec<MrpackFile>,
    pub dependencies: MrpackDependencies,
}

/// Một file cần tải được liệt kê trong `files[]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MrpackFile {
    pub path: String,
    pub hashes: MrpackHashes,
    #[serde(default)]
    pub env: Option<MrpackEnv>,
    pub downloads: Vec<String>,
    pub file_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MrpackHashes {
    pub sha1: String,
    pub sha512: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MrpackEnv {
    #[serde(default)]
    pub client: Option<MrpackEnvSupport>,
    #[serde(default)]
    pub server: Option<MrpackEnvSupport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MrpackEnvSupport {
    Required,
    Optional,
    Unsupported,
}

/// Khai báo phiên bản Minecraft và mod loader trong `modrinth.index.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MrpackDependencies {
    #[serde(default)]
    pub minecraft: Option<String>,
    #[serde(default, rename = "fabric-loader")]
    pub fabric_loader: Option<String>,
    #[serde(default, rename = "quilt-loader")]
    pub quilt_loader: Option<String>,
    #[serde(default)]
    pub forge: Option<String>,
    #[serde(default)]
    pub neoforge: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, String>,
}

/// Phiên bản Minecraft và mod loader suy ra từ `dependencies`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MrpackRuntime {
    pub minecraft: McVersion,
    pub loader: LoaderKind,
    pub loader_version: Option<LoaderVersion>,
}

impl MrpackIndex {
    /// Phân tích JSON và kiểm tra mọi ràng buộc hợp lệ (phiên bản, game, đường dẫn, loader).
    pub fn from_json(text: &str) -> Result<Self, MrpackError> {
        let clean = text.strip_prefix('\u{feff}').unwrap_or(text);
        let index: Self =
            serde_json::from_str(clean).map_err(|e| MrpackError::Json(e.to_string()))?;
        index.validate()?;
        Ok(index)
    }

    /// Kiểm tra lại cấu trúc index và suy ra runtime.
    pub fn validate(&self) -> Result<MrpackRuntime, MrpackError> {
        if self.format_version != MRPACK_FORMAT_VERSION {
            return Err(MrpackError::UnsupportedFormat(self.format_version));
        }
        if self.game != MRPACK_GAME {
            return Err(MrpackError::UnsupportedGame(self.game.clone()));
        }
        for file in &self.files {
            validate_mrpack_path(&file.path)?;
        }
        self.dependencies.resolve()
    }

    pub fn resolve_runtime(&self) -> Result<MrpackRuntime, MrpackError> {
        self.dependencies.resolve()
    }
}

impl MrpackFile {
    /// File được đánh dấu `env.server = "unsupported"` nên bỏ qua trên server (spec P3, mục 3.5).
    pub fn is_client_only(&self) -> bool {
        self.env
            .and_then(|e| e.server)
            .is_some_and(|s| s == MrpackEnvSupport::Unsupported)
    }
}

impl MrpackDependencies {
    /// Suy ra phiên bản Minecraft, loại loader và bản loader từ `dependencies`.
    pub fn resolve(&self) -> Result<MrpackRuntime, MrpackError> {
        if !self.extra.is_empty() {
            return Err(MrpackError::InvalidDependencies(
                "chứa khóa dependency không hỗ trợ",
            ));
        }
        let mc_raw = self
            .minecraft
            .as_deref()
            .ok_or(MrpackError::InvalidDependencies(
                "thiếu phiên bản minecraft",
            ))?;
        let minecraft = McVersion::parse(mc_raw)
            .map_err(|_| MrpackError::InvalidDependencies("phiên bản minecraft không hợp lệ"))?;

        let candidates = [
            (LoaderKind::Fabric, self.fabric_loader.as_deref()),
            (LoaderKind::Quilt, self.quilt_loader.as_deref()),
            (LoaderKind::Forge, self.forge.as_deref()),
            (LoaderKind::NeoForge, self.neoforge.as_deref()),
        ];
        let mut found = candidates
            .into_iter()
            .filter_map(|(kind, ver)| ver.map(|v| (kind, v)));
        let (loader, loader_version) = match found.next() {
            Some((kind, raw)) => {
                if found.next().is_some() {
                    return Err(MrpackError::InvalidDependencies(
                        "khai báo nhiều hơn một mod loader",
                    ));
                }
                let ver = LoaderVersion::parse(raw).map_err(|_| {
                    MrpackError::InvalidDependencies("phiên bản loader không hợp lệ")
                })?;
                (kind, Some(ver))
            }
            None => (LoaderKind::Vanilla, None),
        };
        Ok(MrpackRuntime {
            minecraft,
            loader,
            loader_version,
        })
    }
}

/// Kiểm tra đường dẫn tương đối trong `.mrpack` (cả `files[].path` lẫn entry trong `overrides/`).
///
/// Chặt hơn `SafeRoot::join` một bước: từ chối ngay dẫn đầu `/`, `\`, ổ đĩa/`:`, `.` và `..`
/// thay vì tự chuẩn hóa chúng, để đường dẫn thoát thư mục bị báo lỗi rõ ràng.
pub fn validate_mrpack_path(path: &str) -> Result<(), MrpackError> {
    let err = |reason| {
        Err(MrpackError::InvalidPath {
            path: path.to_owned(),
            reason,
        })
    };
    if path.is_empty() {
        return err("đường dẫn rỗng");
    }
    if path.starts_with('/') {
        return err("đường dẫn không được tuyệt đối");
    }
    if path.contains('\\') {
        return err("không được chứa dấu `\\`");
    }
    if path.contains(':') {
        return err("không được chứa `:` (ổ đĩa hoặc luồng)");
    }
    if path.chars().any(char::is_control) {
        return err("chứa ký tự điều khiển");
    }
    for seg in path.split('/') {
        match seg {
            "" => return err("có đoạn rỗng hoặc dấu `/` thừa"),
            "." => return err("không được chứa `.`"),
            ".." => return err("không được chứa `..`"),
            _ => {}
        }
    }
    Ok(())
}
