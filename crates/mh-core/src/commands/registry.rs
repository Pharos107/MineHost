//! Trích xuất ID vật phẩm, hiệu ứng, sinh vật và phù phép từ file `assets/*/lang/en_us.json`
//! trong `mods/*.jar` (spec `lenh-cua-server.md`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

const MAX_ITEMS: usize = 40_000;
const MAX_OTHER: usize = 5_000;
const MAX_LABEL_CHARS: usize = 100;

/// Một mục trong registry đọc từ mod: `id` dạng `namespace:path`, `label` là tên hiển thị gốc.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub id: String,
    pub label: Option<String>,
}

/// Dữ liệu registry của server đọc từ `mods/*.jar`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerRegistry {
    pub items: Vec<RegistryEntry>,
    pub effects: Vec<RegistryEntry>,
    pub entities: Vec<RegistryEntry>,
    pub enchantments: Vec<RegistryEntry>,
}

/// Đường dẫn file ngôn ngữ tiếng Anh chuẩn trong resource của mod (`assets/<ns>/lang/en_us.json`).
pub fn is_lang_asset_path(entry_name: &str) -> bool {
    let lower = entry_name.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("assets/") else {
        return false;
    };
    let Some((ns, tail)) = rest.split_once('/') else {
        return false;
    };
    !ns.is_empty() && tail == "lang/en_us.json"
}

/// Gom và lọc key chuẩn từ nhiều file `en_us.json` của các mod.
#[derive(Debug, Default)]
pub struct RegistryBuilder {
    items: BTreeMap<String, Option<String>>,
    effects: BTreeMap<String, Option<String>>,
    entities: BTreeMap<String, Option<String>>,
    enchantments: BTreeMap<String, Option<String>>,
}

impl RegistryBuilder {
    pub fn feed_lang_json(&mut self, bytes: &[u8]) {
        // Một số mod trên Windows lưu `en_us.json` kèm UTF-8 BOM khiến `serde_json` báo lỗi ở byte đầu.
        let clean = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
        let Ok(map) = serde_json::from_slice::<BTreeMap<String, serde_json::Value>>(clean) else {
            return;
        };
        for (key, value) in map {
            let Some(raw_label) = value.as_str() else {
                continue;
            };
            let Some((kind, id)) = parse_lang_key(&key) else {
                continue;
            };
            let label = clean_label(raw_label);
            let target = match kind {
                EntryKind::Item => (&mut self.items, MAX_ITEMS),
                EntryKind::Effect => {
                    if id.ends_with(":none") || id.ends_with(":empty") {
                        continue;
                    }
                    (&mut self.effects, MAX_OTHER)
                }
                EntryKind::Entity => (&mut self.entities, MAX_OTHER),
                EntryKind::Enchantment => (&mut self.enchantments, MAX_OTHER),
            };
            if !target.0.contains_key(&id) && target.0.len() >= target.1 {
                continue;
            }
            target.0.entry(id).or_insert(label);
        }
    }

    pub fn build(self) -> ServerRegistry {
        ServerRegistry {
            items: into_entries(self.items),
            effects: into_entries(self.effects),
            entities: into_entries(self.entities),
            enchantments: into_entries(self.enchantments),
        }
    }
}

enum EntryKind {
    Item,
    Effect,
    Entity,
    Enchantment,
}

fn parse_lang_key(key: &str) -> Option<(EntryKind, String)> {
    let (kind, rest) = if let Some(r) = key.strip_prefix("item.") {
        (EntryKind::Item, r)
    } else if let Some(r) = key.strip_prefix("block.") {
        (EntryKind::Item, r)
    } else if let Some(r) = key.strip_prefix("effect.") {
        (EntryKind::Effect, r)
    } else if let Some(r) = key.strip_prefix("entity.") {
        (EntryKind::Entity, r)
    } else {
        (EntryKind::Enchantment, key.strip_prefix("enchantment.")?)
    };

    let (ns, path) = rest.split_once('.')?;
    // Bỏ qua key con như `.tooltip`, `.desc`, `.lore`; chỉ giữ tên định danh gốc của item/effect/entity.
    if ns == "minecraft" || path.contains('.') || !valid_rl_part(ns) || !valid_rl_part(path) {
        return None;
    }
    Some((kind, format!("{ns}:{path}")))
}

fn valid_rl_part(part: &str) -> bool {
    !part.is_empty()
        && part.len() <= 128
        && part.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'/')
        })
}

/// Bỏ mã màu kiểu `§a` của Minecraft và rút gọn khoảng trắng.
fn clean_label(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '§' {
            let _ = chars.next();
            continue;
        }
        out.push(if ch.is_whitespace() { ' ' } else { ch });
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().count() <= MAX_LABEL_CHARS {
        return Some(trimmed.to_owned());
    }
    let mut cut: String = trimmed.chars().take(MAX_LABEL_CHARS).collect();
    cut.push('…');
    Some(cut)
}

fn into_entries(map: BTreeMap<String, Option<String>>) -> Vec<RegistryEntry> {
    map.into_iter()
        .map(|(id, label)| RegistryEntry { id, label })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nhan_dung_duong_dan_en_us_json() {
        assert!(is_lang_asset_path("assets/create/lang/en_us.json"));
        assert!(is_lang_asset_path("assets/AE2/lang/EN_US.JSON"));
        assert!(!is_lang_asset_path("assets/create/lang/vi_vn.json"));
        assert!(!is_lang_asset_path("META-INF/jars/foo.jar"));
        assert!(!is_lang_asset_path("data/create/recipes/x.json"));
    }

    #[test]
    fn loc_dung_key_mod_va_bo_tooltip_vanilla() {
        let mut builder = RegistryBuilder::default();
        builder.feed_lang_json(
            br#"{
                "item.minecraft.diamond": "Diamond",
                "item.create.brass_ingot": "\u00a76Brass Ingot",
                "item.create.wrench.tooltip": "Right click to rotate",
                "block.create.cogwheel": "Cogwheel",
                "effect.irons_spellbooks.hastened": "Hastened",
                "effect.irons_spellbooks.none": "None",
                "entity.alexsmobs.grizzly_bear": "Grizzly Bear",
                "enchantment.ars_nouveau.mana_boost": "Mana Boost",
                "gui.create.menu": "Menu"
            }"#,
        );
        let reg = builder.build();
        assert_eq!(
            reg.items,
            vec![
                RegistryEntry {
                    id: "create:brass_ingot".into(),
                    label: Some("Brass Ingot".into())
                },
                RegistryEntry {
                    id: "create:cogwheel".into(),
                    label: Some("Cogwheel".into())
                },
            ]
        );
        assert_eq!(
            reg.effects,
            vec![RegistryEntry {
                id: "irons_spellbooks:hastened".into(),
                label: Some("Hastened".into())
            }]
        );
        assert_eq!(
            reg.entities,
            vec![RegistryEntry {
                id: "alexsmobs:grizzly_bear".into(),
                label: Some("Grizzly Bear".into())
            }]
        );
        assert_eq!(
            reg.enchantments,
            vec![RegistryEntry {
                id: "ars_nouveau:mana_boost".into(),
                label: Some("Mana Boost".into())
            }]
        );
    }

    #[test]
    fn bo_qua_json_hong() {
        let mut builder = RegistryBuilder::default();
        builder.feed_lang_json(b"{not valid json");
        assert_eq!(builder.build(), ServerRegistry::default());
    }

    #[test]
    fn doc_duoc_json_co_utf8_bom() {
        let mut builder = RegistryBuilder::default();
        builder.feed_lang_json(b"\xEF\xBB\xBF{\"item.create.wrench\": \"Wrench\"}");
        let reg = builder.build();
        assert_eq!(reg.items.len(), 1);
        assert_eq!(reg.items[0].id, "create:wrench");
    }
}
