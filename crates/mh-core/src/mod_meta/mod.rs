//! Đọc metadata của mod (Fabric, Quilt, Forge, NeoForge) từ chuỗi.
//!
//! Mọi hàm nhận trực tiếp nội dung file văn bản đã trích từ jar (không làm I/O ở `mh-core`),
//! áp dụng luật nhận mod chỉ chạy ở client (spec P3, mục 3.5) và chuẩn hóa danh sách dependency.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

mod version_req;

pub use version_req::matches_version_req;

/// Phía chạy được của mod hoặc plugin theo metadata trong jar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModSide {
    Both,
    Client,
    Server,
}

impl ModSide {
    /// Chỉ chạy ở client (bị chuyển sang `mods-disabled/` theo spec P3, mục 3.5).
    pub const fn is_client_only(self) -> bool {
        matches!(self, Self::Client)
    }
}

/// Một dependency khai báo trong metadata của mod hoặc plugin.
///
/// Các dependency nền tảng/loader (`minecraft`, `java`, `fabricloader`, `fabric-loader`,
/// `quilt_loader`, `quilt-loader`, `forge`, `neoforge`, `fml`) và chính `id` của mod đã được
/// lọc bỏ để bước kiểm tra thiếu dependency chỉ đối chiếu với các jar cài thêm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDependency {
    pub id: String,
    pub version_req: String,
    pub required: bool,
}

/// Thông tin đã chuẩn hóa từ metadata trong jar.
///
/// `version` là `None` khi trường phiên bản bị thiếu, rỗng, hoặc là placeholder Maven/Gradle
/// dạng `${...}` (chẳng hạn `${file.jarVersion}`); bước quét jar ở lớp trên sẽ đọc bản thật từ
/// `MANIFEST.MF`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModMeta {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub side: ModSide,
    pub depends: Vec<ModDependency>,
}

/// Định dạng file metadata nằm trong jar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModMetaFormat {
    FabricModJson,
    QuiltModJson,
    ModsToml,
    NeoForgeModsToml,
}

impl ModMetaFormat {
    /// Xác định định dạng từ đường dẫn chuẩn bên trong file jar.
    pub fn from_archive_path(path: &str) -> Option<Self> {
        match path {
            "fabric.mod.json" => Some(Self::FabricModJson),
            "quilt.mod.json" => Some(Self::QuiltModJson),
            "META-INF/mods.toml" => Some(Self::ModsToml),
            "META-INF/neoforge.mods.toml" => Some(Self::NeoForgeModsToml),
            _ => None,
        }
    }
}

/// Lỗi khi phân tích metadata mod.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModMetaError {
    #[error("JSON metadata không hợp lệ: {0}")]
    Json(String),
    #[error("TOML metadata không hợp lệ: {0}")]
    Toml(String),
    #[error("metadata thiếu trường bắt buộc `{0}`")]
    MissingField(&'static str),
}

/// Đọc metadata theo định dạng đã chọn.
pub fn parse_mod_meta(format: ModMetaFormat, text: &str) -> Result<ModMeta, ModMetaError> {
    match format {
        ModMetaFormat::FabricModJson => parse_fabric_mod_json(text),
        ModMetaFormat::QuiltModJson => parse_quilt_mod_json(text),
        ModMetaFormat::ModsToml | ModMetaFormat::NeoForgeModsToml => parse_forge_toml(text),
    }
}

fn parse_json(text: &str) -> Result<Value, ModMetaError> {
    serde_json::from_str(strip_bom(text)).map_err(|e| ModMetaError::Json(e.to_string()))
}

fn parse_fabric_mod_json(text: &str) -> Result<ModMeta, ModMetaError> {
    let root = parse_json(text)?;
    let id = require_field(root.get("id").and_then(Value::as_str), "id")?;
    let name =
        opt_non_empty(root.get("name").and_then(Value::as_str)).unwrap_or_else(|| id.clone());
    let side = env_to_side(root.get("environment").and_then(Value::as_str));
    let mut depends = Vec::new();
    if let Some(map) = root.get("depends").and_then(Value::as_object) {
        for (dep_id, val) in map {
            if !is_builtin_dependency(dep_id, &id) {
                depends.push(ModDependency {
                    id: dep_id.clone(),
                    version_req: json_req_string(Some(val)),
                    required: true,
                });
            }
        }
    }
    Ok(ModMeta {
        id,
        name,
        version: normalize_version(root.get("version").and_then(Value::as_str)),
        side,
        depends,
    })
}

fn parse_quilt_mod_json(text: &str) -> Result<ModMeta, ModMetaError> {
    let root = parse_json(text)?;
    let loader = root
        .get("quilt_loader")
        .and_then(Value::as_object)
        .ok_or(ModMetaError::MissingField("quilt_loader"))?;
    let id = require_field(loader.get("id").and_then(Value::as_str), "id")?;
    let name = loader
        .get("metadata")
        .and_then(|m| m.get("name"))
        .and_then(Value::as_str)
        .and_then(|s| opt_non_empty(Some(s)))
        .unwrap_or_else(|| id.clone());
    let env = root
        .get("minecraft")
        .or_else(|| loader.get("minecraft"))
        .and_then(|m| m.get("environment"))
        .and_then(Value::as_str);
    let mut depends = Vec::new();
    if let Some(entries) = loader.get("depends").and_then(Value::as_array) {
        for entry in entries {
            push_quilt_dep(&mut depends, entry, &id, false);
        }
    }
    Ok(ModMeta {
        id,
        name,
        version: normalize_version(loader.get("version").and_then(Value::as_str)),
        side: env_to_side(env),
        depends,
    })
}

fn push_quilt_dep(depends: &mut Vec<ModDependency>, entry: &Value, self_id: &str, in_group: bool) {
    match entry {
        Value::String(raw_id) => {
            if let Some(dep_id) = opt_non_empty(Some(raw_id))
                && !is_builtin_dependency(&dep_id, self_id)
            {
                depends.push(ModDependency {
                    id: dep_id,
                    version_req: "*".to_owned(),
                    required: !in_group,
                });
            }
        }
        // Quilt cho phép phần tử của `depends` là mảng ("cần ít nhất một trong các mod này").
        // `ModDependency` không biểu diễn được nhóm OR và preflight chỉ cảnh báo, nên hạ từng
        // mod trong nhóm xuống `required: false` để tránh báo thiếu sai.
        Value::Array(group) => {
            for item in group {
                push_quilt_dep(depends, item, self_id, true);
            }
        }
        Value::Object(map) => {
            let Some(dep_id) = opt_non_empty(map.get("id").and_then(Value::as_str)) else {
                return;
            };
            if is_builtin_dependency(&dep_id, self_id) {
                return;
            }
            let optional = map
                .get("optional")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            depends.push(ModDependency {
                id: dep_id,
                version_req: json_req_string(map.get("versions")),
                required: !in_group && !optional,
            });
        }
        // Bỏ qua phần tử có hình dạng lạ thay vì làm hỏng cả file.
        _ => {}
    }
}

/// Đọc `META-INF/mods.toml` hoặc `META-INF/neoforge.mods.toml`.
///
/// Nếu jar khai báo nhiều `[[mods]]` (ví dụ kèm stub tương thích), chỉ lấy phần tử đầu tiên
/// làm mod chính vì một file jar tương ứng một mục `ContentFile`.
fn parse_forge_toml(text: &str) -> Result<ModMeta, ModMetaError> {
    let raw: RawForgeToml =
        toml::from_str(strip_bom(text)).map_err(|e| ModMetaError::Toml(e.to_string()))?;
    let primary = raw
        .mods
        .into_iter()
        .next()
        .ok_or(ModMetaError::MissingField("mods"))?;
    let id = require_field(primary.mod_id.as_deref(), "id")?;
    let name = opt_non_empty(primary.display_name.as_deref()).unwrap_or_else(|| id.clone());
    let raw_deps: Vec<&RawForgeDep> = raw
        .dependencies
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(&id))
        .map(|(_, list)| list.as_slice().iter().collect())
        .unwrap_or_else(|| {
            raw.dependencies
                .values()
                .flat_map(|l| l.as_slice().iter())
                .collect()
        });
    let side = detect_forge_side(&raw_deps);
    let mut depends = Vec::new();
    for dep in raw_deps {
        if dep.dep_type.as_deref().is_some_and(|t| {
            t.eq_ignore_ascii_case("incompatible") || t.eq_ignore_ascii_case("discouraged")
        }) || is_builtin_dependency(&dep.mod_id, &id)
        {
            continue;
        }
        let required = dep.mandatory.unwrap_or_else(|| {
            dep.dep_type
                .as_deref()
                .is_none_or(|t| !t.eq_ignore_ascii_case("optional"))
        });
        depends.push(ModDependency {
            id: dep.mod_id.clone(),
            version_req: opt_non_empty(dep.version_range.as_deref())
                .unwrap_or_else(|| "*".to_owned()),
            required,
        });
    }
    Ok(ModMeta {
        id,
        name,
        version: normalize_version(primary.version.as_deref()),
        side,
        depends,
    })
}

/// Theo spec P3 mục 3.5: mọi dependency vào `minecraft`, `forge` hoặc `neoforge` đều có
/// `side = "CLIENT"` (và danh sách không rỗng) thì mod chỉ chạy ở client.
fn detect_forge_side(deps: &[&RawForgeDep]) -> ModSide {
    let sides: Vec<&str> = deps
        .iter()
        .filter(|d| {
            matches!(
                d.mod_id.to_ascii_lowercase().as_str(),
                "minecraft" | "forge" | "neoforge"
            )
        })
        .map(|d| d.side.as_deref().unwrap_or("BOTH").trim())
        .collect();
    if sides.is_empty() {
        return ModSide::Both;
    }
    if sides.iter().all(|s| s.eq_ignore_ascii_case("CLIENT")) {
        ModSide::Client
    } else if sides.iter().all(|s| s.eq_ignore_ascii_case("SERVER")) {
        ModSide::Server
    } else {
        ModSide::Both
    }
}

fn env_to_side(env: Option<&str>) -> ModSide {
    match env.map(str::trim) {
        Some(s) if s.eq_ignore_ascii_case("client") => ModSide::Client,
        Some(s)
            if s.eq_ignore_ascii_case("server") || s.eq_ignore_ascii_case("dedicated_server") =>
        {
            ModSide::Server
        }
        _ => ModSide::Both,
    }
}

fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

fn opt_non_empty(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

fn normalize_version(raw: Option<&str>) -> Option<String> {
    let trimmed = opt_non_empty(raw)?;
    // Placeholder Maven/Gradle chưa được thay khi đóng gói (ví dụ `${file.jarVersion}`).
    if let Some(start) = trimmed.find("${")
        && trimmed[start + 2..].contains('}')
    {
        return None;
    }
    Some(trimmed)
}

fn require_field(s: Option<&str>, field: &'static str) -> Result<String, ModMetaError> {
    opt_non_empty(s).ok_or(ModMetaError::MissingField(field))
}

fn is_builtin_dependency(dep_id: &str, self_id: &str) -> bool {
    dep_id.eq_ignore_ascii_case(self_id)
        || matches!(
            dep_id.to_ascii_lowercase().as_str(),
            "minecraft"
                | "java"
                | "fabricloader"
                | "fabric-loader"
                | "quilt_loader"
                | "quilt-loader"
                | "forge"
                | "neoforge"
                | "fml"
        )
}

fn json_req_string(val: Option<&Value>) -> String {
    match val {
        Some(Value::String(s)) => opt_non_empty(Some(s)).unwrap_or_else(|| "*".to_owned()),
        Some(Value::Array(arr)) => join_string_array(arr, " || "),
        Some(Value::Object(map)) => {
            if let Some(Value::Array(arr)) = map.get("all") {
                join_string_array(arr, " ")
            } else if let Some(Value::Array(arr)) = map.get("any") {
                join_string_array(arr, " || ")
            } else {
                "*".to_owned()
            }
        }
        _ => "*".to_owned(),
    }
}

fn join_string_array(arr: &[Value], sep: &str) -> String {
    let parts: Vec<&str> = arr
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        "*".to_owned()
    } else {
        parts.join(sep)
    }
}

#[derive(Deserialize)]
struct RawForgeToml {
    #[serde(default)]
    mods: Vec<RawForgeMod>,
    #[serde(default)]
    dependencies: BTreeMap<String, RawDepList>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawForgeMod {
    mod_id: Option<String>,
    version: Option<String>,
    display_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawDepList {
    Many(Vec<RawForgeDep>),
    One(RawForgeDep),
}

impl RawDepList {
    fn as_slice(&self) -> &[RawForgeDep] {
        match self {
            Self::Many(v) => v,
            Self::One(item) => std::slice::from_ref(item),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawForgeDep {
    mod_id: String,
    mandatory: Option<bool>,
    #[serde(rename = "type")]
    dep_type: Option<String>,
    version_range: Option<String>,
    side: Option<String>,
}
