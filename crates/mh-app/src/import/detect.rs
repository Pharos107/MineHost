//! Nhận loại server và phiên bản của một thư mục có sẵn (spec P2, mục 6.11).
//!
//! Đọc theo thứ tự: `version_history.json` mà Paper và Purpur ghi lúc chạy, rồi `version.json`
//! trong jar. Không nhận ra thì để trống cho người dùng chọn tay.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use mh_core::instance::JarName;
use mh_core::{LoaderKind, McVersion};
use serde::Deserialize;

/// JSON trong thư mục server hoặc trong jar chỉ vài trăm byte; đọc nhiều hơn là file lạ.
const SMALL_JSON: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    pub loader: Option<LoaderKind>,
    pub minecraft: Option<McVersion>,
    /// Jar sẽ chạy, nằm ngay trong thư mục. Không có thì không nhập được.
    pub jar: Option<JarName>,
    /// Có thư mục `mods/`: server có mod chưa hỗ trợ ở P2.
    pub has_mods: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionHistory {
    current_version: String,
}

#[derive(Deserialize)]
struct VersionJson {
    id: String,
}

/// Không đọc được file nào thì trả kết quả trống, không báo lỗi: người dùng vẫn chọn tay được.
pub fn inspect(dir: &Path) -> Detected {
    let (history_loader, history_version) = read_history(dir);
    let (jar, jar_loader, jar_version) = pick_jar(dir);
    Detected {
        loader: history_loader.or(jar_loader),
        minecraft: history_version.or(jar_version),
        jar,
        has_mods: dir.join("mods").is_dir(),
    }
}

/// `"git-Paper-232 (MC: 1.21.4)"`, `"git-Purpur-2416 (MC: 1.21.4)"`. Paper mới ghi
/// `"1.21.4-232-abc1234 (MC: 1.21.4)"`, không có tên loại: khi đó lấy loại từ jar.
pub(crate) fn parse_history(current: &str) -> (Option<LoaderKind>, Option<McVersion>) {
    let lower = current.to_ascii_lowercase();
    let loader = if lower.contains("purpur") {
        Some(LoaderKind::Purpur)
    } else if lower.contains("paper") {
        Some(LoaderKind::Paper)
    } else {
        None
    };
    let version = current
        .split_once("(MC: ")
        .and_then(|(_, rest)| rest.split_once(')'))
        .and_then(|(id, _)| McVersion::parse(id.trim()).ok());
    (loader, version)
}

fn read_history(dir: &Path) -> (Option<LoaderKind>, Option<McVersion>) {
    let Some(text) = read_small(&dir.join("version_history.json")) else {
        return (None, None);
    };
    match serde_json::from_str::<VersionHistory>(&text) {
        Ok(history) => parse_history(&history.current_version),
        Err(_) => (None, None),
    }
}

fn read_small(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut text = String::new();
    file.take(SMALL_JSON).read_to_string(&mut text).ok()?;
    Some(text)
}

/// Jar ngay trong thư mục, ưu tiên jar nhận ra được loại, rồi tới `server.jar`.
fn pick_jar(dir: &Path) -> (Option<JarName>, Option<LoaderKind>, Option<McVersion>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (None, None, None);
    };
    let mut jars: Vec<JarName> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|name| JarName::parse(&name).ok())
        .collect();
    jars.sort_by_key(|jar| (jar.as_str() != "server.jar", jar.as_str().to_owned()));
    let mut fallback = None;
    for jar in jars {
        let (loader, version) = inspect_jar(&dir.join(jar.as_str()), jar.as_str());
        if loader.is_some() {
            return (Some(jar), loader, version);
        }
        fallback.get_or_insert((jar, version));
    }
    match fallback {
        Some((jar, version)) => (Some(jar), None, version),
        None => (None, None, None),
    }
}

/// Paper và Purpur đóng gói bằng paperclip; jar của Mojang từ 1.18 có `net/minecraft/bundler/`,
/// bản cũ hơn có thẳng `net/minecraft/server/`. Purpur chỉ phân biệt được với Paper qua tên file.
fn inspect_jar(path: &Path, name: &str) -> (Option<LoaderKind>, Option<McVersion>) {
    let Ok(file) = File::open(path) else {
        return (None, None);
    };
    let Ok(mut zip) = zip::ZipArchive::new(file) else {
        return (None, None);
    };
    let names: Vec<String> = zip.file_names().map(str::to_owned).collect();
    let has = |prefix: &str| names.iter().any(|n| n.starts_with(prefix));
    let loader = if has("io/papermc/paperclip/") {
        if name.to_ascii_lowercase().contains("purpur") {
            Some(LoaderKind::Purpur)
        } else {
            Some(LoaderKind::Paper)
        }
    } else if has("net/minecraft/bundler/") || has("net/minecraft/server/") {
        Some(LoaderKind::Vanilla)
    } else {
        None
    };
    let version = zip
        .by_name("version.json")
        .ok()
        .and_then(|entry| {
            let mut text = String::new();
            entry.take(SMALL_JSON).read_to_string(&mut text).ok()?;
            serde_json::from_str::<VersionJson>(&text).ok()
        })
        .and_then(|json| McVersion::parse(&json.id).ok());
    (loader, version)
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod tests;
