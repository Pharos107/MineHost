//! Quét tên vật phẩm, hiệu ứng, sinh vật, phù phép từ `mods/*.jar` (spec `lenh-cua-server.md`).

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use mh_core::commands::{RegistryBuilder, RegistryEntry, ServerRegistry, is_lang_asset_path};
use mh_core::{ErrorCode, ServerId};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use zip::ZipArchive;

use crate::{App, AppError};

const FILE: &str = "registry.json";
const MAX_LANG_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryList {
    #[serde(with = "time::serde::rfc3339::option")]
    pub scanned_at: Option<OffsetDateTime>,
    pub items: Vec<RegistryEntry>,
    pub effects: Vec<RegistryEntry>,
    pub entities: Vec<RegistryEntry>,
    pub enchantments: Vec<RegistryEntry>,
}

impl From<(OffsetDateTime, ServerRegistry)> for RegistryList {
    fn from((at, reg): (OffsetDateTime, ServerRegistry)) -> Self {
        Self {
            scanned_at: Some(at),
            items: reg.items,
            effects: reg.effects,
            entities: reg.entities,
            enchantments: reg.enchantments,
        }
    }
}

impl App {
    /// Trả về registry đã cache trong `registry.json`; nếu chưa có và thư mục `mods/` tồn tại thì
    /// quét một lần rồi lưu lại.
    pub async fn server_registry(&self, id: ServerId) -> Result<RegistryList, AppError> {
        self.store.load(id)?;
        let paths = self.store.paths(id);
        let file = paths.root.join(FILE);
        match std::fs::read(&file) {
            Ok(bytes) => {
                if let Ok(list) = serde_json::from_slice::<RegistryList>(&bytes) {
                    return Ok(list);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(AppError::new(
                    ErrorCode::Internal,
                    format!("đọc {FILE}: {error}"),
                ));
            }
        }
        let mods_dir = paths.server_dir.join("mods");
        if !mods_dir.is_dir() {
            return Ok(RegistryList::default());
        }
        self.scan_registry(id).await
    }

    /// Quét lại toàn bộ `mods/*.jar` (chạy trên thread chặn I/O) rồi lưu vào `registry.json`.
    pub async fn scan_registry(&self, id: ServerId) -> Result<RegistryList, AppError> {
        self.store.load(id)?;
        let paths = self.store.paths(id);
        let mods_dir = paths.server_dir.join("mods");
        let reg = tokio::task::spawn_blocking(move || scan_mods_dir(&mods_dir))
            .await
            .map_err(|error| AppError::new(ErrorCode::Internal, error.to_string()))?;
        let list = RegistryList::from((OffsetDateTime::now_utc(), reg));
        let bytes = serde_json::to_vec_pretty(&list)
            .map_err(|error| AppError::new(ErrorCode::Internal, error.to_string()))?;
        mh_platform::fs::write_atomic(&paths.root.join(FILE), &bytes)?;
        tracing::info!(
            server = %id,
            items = list.items.len(),
            effects = list.effects.len(),
            entities = list.entities.len(),
            enchantments = list.enchantments.len(),
            "đã quét registry từ mods/"
        );
        Ok(list)
    }
}

fn scan_mods_dir(mods_dir: &Path) -> ServerRegistry {
    let Ok(entries) = std::fs::read_dir(mods_dir) else {
        return ServerRegistry::default();
    };
    let mut builder = RegistryBuilder::default();
    for entry in entries.flatten() {
        let path = entry.path();
        let is_jar = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("jar"));
        if !is_jar || !path.is_file() {
            continue;
        }
        read_mod_jar(&path, &mut builder);
    }
    builder.build()
}

fn read_mod_jar(path: &Path, builder: &mut RegistryBuilder) {
    let Ok(file) = File::open(path) else {
        return;
    };
    let Ok(mut archive) = ZipArchive::new(BufReader::new(file)) else {
        return;
    };
    let targets: Vec<usize> = (0..archive.len())
        .filter(|&i| {
            archive
                .name_for_index(i)
                .is_some_and(|res| res.is_ok_and(|name| is_lang_asset_path(&name)))
        })
        .collect();
    for index in targets {
        let Ok(entry) = archive.by_index(index) else {
            continue;
        };
        let mut buf = Vec::new();
        if entry.take(MAX_LANG_BYTES).read_to_end(&mut buf).is_ok() {
            builder.feed_lang_json(&buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;

    use super::*;

    fn write_jar(path: &Path, files: &[(&str, &str)]) {
        let file = File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = SimpleFileOptions::default();
        for (name, body) in files {
            zip.start_file(name, opts).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn quet_en_us_tu_nhieu_jar_bo_qua_file_hong() {
        let tmp = tempfile::tempdir().unwrap();
        write_jar(
            &tmp.path().join("create.jar"),
            &[(
                "assets/create/lang/en_us.json",
                r#"{"item.create.brass_ingot": "Brass Ingot", "block.create.cogwheel": "Cogwheel"}"#,
            )],
        );
        write_jar(
            &tmp.path().join("spells.jar"),
            &[(
                "assets/spells/lang/en_us.json",
                r#"{"effect.spells.haste": "Haste", "entity.spells.wisp": "Wisp", "enchantment.spells.mana": "Mana"}"#,
            )],
        );
        std::fs::write(tmp.path().join("broken.jar"), b"not a zip").unwrap();
        std::fs::write(tmp.path().join("readme.txt"), b"ignore").unwrap();

        let reg = scan_mods_dir(tmp.path());
        assert_eq!(reg.items.len(), 2);
        assert_eq!(reg.effects[0].id, "spells:haste");
        assert_eq!(reg.entities[0].id, "spells:wisp");
        assert_eq!(reg.enchantments[0].id, "spells:mana");
    }
}
