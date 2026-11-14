//! Định dạng `manifest.json` của modpack CurseForge (`.zip`).
//!
//! Crate này không làm I/O, nên chỉ định nghĩa kiểu dữ liệu serde và các hàm thuần kiểm tra
//! tên thư mục `overrides`, phiên bản định dạng, và suy ra loader + Minecraft từ `modLoaders`.

use serde::{Deserialize, Serialize};

use crate::mrpack::{MrpackError, validate_mrpack_path};
use crate::version::{LoaderKind, LoaderVersion, McVersion};

pub const CF_MANIFEST_TYPE: &str = "minecraftModpack";
pub const CF_MANIFEST_VERSION: u32 = 1;

/// Lỗi khi phân tích hoặc kiểm tra `manifest.json` của CurseForge.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CfManifestError {
    #[error("manifest.json không hợp lệ: {0}")]
    Json(String),
    #[error("manifestType `{0}` không được hỗ trợ (chỉ nhận minecraftModpack)")]
    UnsupportedType(String),
    #[error("manifestVersion {0} không được hỗ trợ (chỉ nhận 1)")]
    UnsupportedVersion(u32),
    #[error("thư mục overrides `{path}` không hợp lệ: {reason}")]
    InvalidOverrides { path: String, reason: &'static str },
    #[error("cấu hình minecraft/modLoaders trong manifest.json không hợp lệ: {0}")]
    InvalidRuntime(&'static str),
}

/// Nội dung `manifest.json` ở gốc gói modpack CurseForge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfManifest {
    pub minecraft: CfMinecraft,
    #[serde(default)]
    pub manifest_type: Option<String>,
    #[serde(default)]
    pub manifest_version: Option<u32>,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub files: Vec<CfManifestFile>,
    #[serde(default = "default_overrides")]
    pub overrides: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfMinecraft {
    pub version: String,
    #[serde(default)]
    pub mod_loaders: Vec<CfModLoader>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfModLoader {
    pub id: String,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfManifestFile {
    #[serde(rename = "projectID")]
    pub project_id: u64,
    #[serde(rename = "fileID")]
    pub file_id: u64,
    #[serde(default = "default_required")]
    pub required: bool,
}

/// Phiên bản Minecraft và mod loader suy ra từ `minecraft` trong `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfRuntime {
    pub minecraft: McVersion,
    pub loader: LoaderKind,
    pub loader_version: Option<LoaderVersion>,
}

fn default_overrides() -> String {
    "overrides".to_owned()
}

const fn default_required() -> bool {
    true
}

impl CfManifest {
    /// Phân tích JSON và kiểm tra mọi ràng buộc hợp lệ (kiểu manifest, `overrides`, loader).
    pub fn from_json(text: &str) -> Result<Self, CfManifestError> {
        let clean = text.strip_prefix('\u{feff}').unwrap_or(text);
        let manifest: Self =
            serde_json::from_str(clean).map_err(|e| CfManifestError::Json(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Kiểm tra tính hợp lệ của manifest và suy ra runtime.
    pub fn validate(&self) -> Result<CfRuntime, CfManifestError> {
        if let Some(kind) = &self.manifest_type
            && kind != CF_MANIFEST_TYPE
        {
            return Err(CfManifestError::UnsupportedType(kind.clone()));
        }
        if let Some(ver) = self.manifest_version
            && ver != CF_MANIFEST_VERSION
        {
            return Err(CfManifestError::UnsupportedVersion(ver));
        }
        validate_cf_overrides(&self.overrides)?;
        self.resolve_runtime()
    }

    /// Suy ra phiên bản Minecraft, loại loader và bản loader từ `minecraft.modLoaders`.
    pub fn resolve_runtime(&self) -> Result<CfRuntime, CfManifestError> {
        let minecraft = McVersion::parse(&self.minecraft.version)
            .map_err(|_| CfManifestError::InvalidRuntime("phiên bản minecraft không hợp lệ"))?;
        let selected = pick_mod_loader(&self.minecraft.mod_loaders)?;
        let (loader, loader_version) = match selected {
            None => (LoaderKind::Vanilla, None),
            Some(raw) => {
                let (kind, ver) = parse_loader_id(&raw.id, &minecraft)?;
                (kind, Some(ver))
            }
        };
        Ok(CfRuntime {
            minecraft,
            loader,
            loader_version,
        })
    }
}

fn pick_mod_loader(loaders: &[CfModLoader]) -> Result<Option<&CfModLoader>, CfManifestError> {
    if loaders.is_empty() {
        return Ok(None);
    }
    let mut primaries = loaders.iter().filter(|l| l.primary);
    match primaries.next() {
        Some(first) => {
            if primaries.next().is_some() {
                return Err(CfManifestError::InvalidRuntime(
                    "có nhiều hơn một mod loader primary",
                ));
            }
            Ok(Some(first))
        }
        None if loaders.len() == 1 => Ok(Some(&loaders[0])),
        None => Err(CfManifestError::InvalidRuntime(
            "có nhiều mod loader nhưng không đánh dấu primary",
        )),
    }
}

fn parse_loader_id(
    id: &str,
    mc: &McVersion,
) -> Result<(LoaderKind, LoaderVersion), CfManifestError> {
    let (kind, rest) = if let Some(v) = id.strip_prefix("forge-") {
        // Vài pack cũ ghi `forge-<mc>-<loader>`; bỏ tiền tố `<mc>-` nếu có.
        let mc_prefix = format!("{mc}-");
        (
            LoaderKind::Forge,
            v.strip_prefix(mc_prefix.as_str()).unwrap_or(v),
        )
    } else if let Some(v) = id.strip_prefix("neoforge-") {
        (LoaderKind::NeoForge, v)
    } else if let Some(v) = id
        .strip_prefix("fabric-loader-")
        .or_else(|| id.strip_prefix("fabric-"))
    {
        (LoaderKind::Fabric, v)
    } else if let Some(v) = id
        .strip_prefix("quilt-loader-")
        .or_else(|| id.strip_prefix("quilt-"))
    {
        (LoaderKind::Quilt, v)
    } else {
        return Err(CfManifestError::InvalidRuntime("mod loader chưa hỗ trợ"));
    };
    let ver = LoaderVersion::parse(rest)
        .map_err(|_| CfManifestError::InvalidRuntime("phiên bản mod loader không hợp lệ"))?;
    Ok((kind, ver))
}

/// Kiểm tra tên thư mục `overrides` của pack CurseForge phải là tên một thư mục gốc an toàn.
pub fn validate_cf_overrides(dir: &str) -> Result<(), CfManifestError> {
    if let Err(MrpackError::InvalidPath { reason, .. }) = validate_mrpack_path(dir) {
        return Err(CfManifestError::InvalidOverrides {
            path: dir.to_owned(),
            reason,
        });
    }
    if dir.contains('/') {
        return Err(CfManifestError::InvalidOverrides {
            path: dir.to_owned(),
            reason: "phải là tên một thư mục gốc, không chứa `/`",
        });
    }
    Ok(())
}
