//! Đọc `provides` và danh sách jar lồng (jar-in-jar) từ metadata mod.
//!
//! Loader coi id trong `provides` và mod trong jar lồng là đã có, nên preflight cũng phải tính
//! chúng; thiếu bước này thì Fabric API (`provides: ["fabric"]`) hay Trinkets (mang sẵn
//! cardinal-components) bị báo thiếu sai.

use serde::Deserialize;
use serde_json::Value;

use super::{ModMetaError, strip_bom};

/// `provides` của Fabric (mảng chuỗi) hoặc Quilt (chuỗi hoặc `{ "id": ... }`).
pub(super) fn json_provides(val: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(items)) = val else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| match item {
            Value::String(s) => Some(s.as_str()),
            Value::Object(map) => map.get("id").and_then(Value::as_str),
            _ => None,
        })
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `jars` của Fabric (`[{ "file": ... }]`) hoặc Quilt (`["path"]`, có bản dùng `{ "file": ... }`).
pub(super) fn json_jars(val: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(items)) = val else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| match item {
            Value::String(s) => Some(s.as_str()),
            Value::Object(map) => map.get("file").and_then(Value::as_str),
            _ => None,
        })
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

#[derive(Deserialize)]
struct RawJarJar {
    #[serde(default)]
    jars: Vec<RawJarJarEntry>,
}

#[derive(Deserialize)]
struct RawJarJarEntry {
    path: Option<String>,
}

/// Đọc `META-INF/jarjar/metadata.json` (jar lồng của Forge/NeoForge), trả đường dẫn các jar con.
pub fn parse_jarjar_metadata(text: &str) -> Result<Vec<String>, ModMetaError> {
    let raw: RawJarJar =
        serde_json::from_str(strip_bom(text)).map_err(|e| ModMetaError::Json(e.to_string()))?;
    Ok(raw
        .jars
        .into_iter()
        .filter_map(|j| j.path)
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect())
}
