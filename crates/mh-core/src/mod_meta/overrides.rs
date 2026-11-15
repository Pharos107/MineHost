//! Ghi đè dependency của Fabric loader: `config/fabric_loader_dependencies.json` (version 1).
//!
//! Modpack dùng file này để bỏ hoặc thêm dependency của một mod mà không sửa jar (Prominence II
//! bỏ `modmenu` của Forge Config Screens trên server). Fabric loader áp dụng nó trước khi kiểm tra
//! dependency, nên preflight cũng phải áp dụng, nếu không sẽ báo thiếu sai.

use std::collections::HashMap;

use serde_json::{Map, Value};

use super::{ModDependency, ModMeta, ModMetaError, is_builtin_dependency, json_req_string};

/// Các ghi đè đã đọc, theo id mod (chữ thường).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FabricDepOverrides {
    by_mod: HashMap<String, DepOverride>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DepOverride {
    /// `"depends"`: thay toàn bộ danh sách.
    replace: Option<Vec<ModDependency>>,
    /// `"+depends"`: thêm (hoặc đổi ràng buộc phiên bản).
    add: Vec<ModDependency>,
    /// `"-depends"`: bỏ theo id; giá trị bị Fabric bỏ qua.
    remove: Vec<String>,
}

/// Đọc nội dung `fabric_loader_dependencies.json`. Chỉ hiểu `version: 1`, như Fabric loader 0.14+.
pub fn parse_fabric_dep_overrides(text: &str) -> Result<FabricDepOverrides, ModMetaError> {
    let root = super::parse_json(text)?;
    if root.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(ModMetaError::MissingField("version"));
    }
    let mut by_mod = HashMap::new();
    let Some(mods) = root.get("overrides").and_then(Value::as_object) else {
        return Ok(FabricDepOverrides { by_mod });
    };
    for (mod_id, entry) in mods {
        let Some(entry) = entry.as_object() else {
            continue;
        };
        let parsed = DepOverride {
            replace: entry
                .get("depends")
                .and_then(Value::as_object)
                .map(|m| deps_from(m, mod_id)),
            add: entry
                .get("+depends")
                .and_then(Value::as_object)
                .map(|m| deps_from(m, mod_id))
                .unwrap_or_default(),
            remove: entry
                .get("-depends")
                .and_then(Value::as_object)
                .map(|m| m.keys().map(|k| k.to_ascii_lowercase()).collect())
                .unwrap_or_default(),
        };
        by_mod.insert(mod_id.to_ascii_lowercase(), parsed);
    }
    Ok(FabricDepOverrides { by_mod })
}

fn deps_from(map: &Map<String, Value>, self_id: &str) -> Vec<ModDependency> {
    map.iter()
        .filter(|(id, _)| !is_builtin_dependency(id, self_id))
        .map(|(id, val)| ModDependency {
            id: id.clone(),
            version_req: json_req_string(Some(val)),
            required: true,
        })
        .collect()
}

impl FabricDepOverrides {
    /// Danh sách dependency của `meta` sau khi áp ghi đè (thứ tự: thay, bỏ, thêm).
    pub fn effective_depends(&self, meta: &ModMeta) -> Vec<ModDependency> {
        let Some(over) = self.by_mod.get(&meta.id.to_ascii_lowercase()) else {
            return meta.depends.clone();
        };
        let mut deps = over.replace.clone().unwrap_or_else(|| meta.depends.clone());
        deps.retain(|d| !over.remove.contains(&d.id.to_ascii_lowercase()));
        for added in &over.add {
            deps.retain(|d| !d.id.eq_ignore_ascii_case(&added.id));
            deps.push(added.clone());
        }
        deps
    }
}
