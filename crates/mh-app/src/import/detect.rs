//! Nhận loại server và phiên bản của một thư mục có sẵn (spec P2, mục 6.11).
//!
//! Đọc theo thứ tự: `version_history.json` mà Paper và Purpur ghi lúc chạy, rồi `version.json`
//! trong jar. Không nhận ra thì để trống cho người dùng chọn tay.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use mh_core::instance::JarName;
use mh_core::{ArgsFilePath, Launch, LoaderKind, LoaderVersion, McVersion, neoforge_minecraft};
use serde::Deserialize;

/// JSON trong thư mục server hoặc trong jar chỉ vài trăm byte; đọc nhiều hơn là file lạ.
const SMALL_JSON: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    pub loader: Option<LoaderKind>,
    pub loader_version: Option<LoaderVersion>,
    pub minecraft: Option<McVersion>,
    /// Jar sẽ chạy, nằm ngay trong thư mục. Không có thì không nhập được (trừ `args_file`).
    pub jar: Option<JarName>,
    /// Cách chạy nhận ra từ thư mục (`ArgsFile` cho Forge/NeoForge, `Jar` cho phần còn lại).
    pub launch: Option<Launch>,
    /// Có thư mục `mods/`: chỉ nhập được khi chọn mod loader.
    pub has_mods: bool,
    /// Có `user_jvm_args.txt` chứa cờ JVM ngoài chú thích `#` (MineHost không đọc file này).
    pub ignored_jvm_args: bool,
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

/// Kiểm tra `user_jvm_args.txt` có dòng nào chứa cờ ngoài chú thích `#`.
pub fn has_ignored_jvm_args(dir: &Path) -> bool {
    let Some(text) = read_small(&dir.join("user_jvm_args.txt")) else {
        return false;
    };
    text.lines()
        .any(|line| !line.split('#').next().unwrap_or("").trim().is_empty())
}

/// Không đọc được file nào thì trả kết quả trống, không báo lỗi: người dùng vẫn chọn tay được.
pub fn inspect(dir: &Path) -> Detected {
    let has_mods = dir.join("mods").is_dir();
    let ignored_jvm_args = has_ignored_jvm_args(dir);
    if let Some((loader, loader_version, minecraft, launch)) = detect_args_file(dir) {
        return Detected {
            loader: Some(loader),
            loader_version,
            minecraft,
            jar: None,
            launch: Some(launch),
            has_mods,
            ignored_jvm_args,
        };
    }
    let (history_loader, history_version) = read_history(dir);
    let JarPick {
        jar,
        loader: jar_loader,
        minecraft: jar_mc,
        loader_version: jar_lv,
    } = pick_jar(dir);
    let loader = history_loader.or(jar_loader);
    let minecraft = history_version
        .or(jar_mc)
        .or_else(|| detect_mc_from_dirs(dir));
    let loader_version = jar_lv.or_else(|| loader.and_then(|l| detect_loader_from_dirs(dir, l)));
    let launch = jar.clone().map(Launch::Jar);
    Detected {
        loader,
        loader_version,
        minecraft,
        jar,
        launch,
        has_mods,
        ignored_jvm_args,
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

/// Jar ngay trong thư mục sẽ chạy, kèm loại, bản Minecraft và bản loader nhận ra được.
#[derive(Debug, Default)]
struct JarPick {
    jar: Option<JarName>,
    loader: Option<LoaderKind>,
    minecraft: Option<McVersion>,
    loader_version: Option<LoaderVersion>,
}

/// Thứ tự ưu tiên khi thư mục có nhiều jar: jar khởi động mod loader đứng trước `server.jar` của
/// Mojang nằm cạnh nó (Fabric, Quilt cài bằng installer; Forge trước 1.17), rồi tới Paper/Purpur,
/// Vanilla, cuối cùng là jar không nhận ra.
fn jar_rank(loader: Option<LoaderKind>) -> u8 {
    match loader {
        Some(LoaderKind::Vanilla) => 2,
        Some(kind) if kind.is_mod_loader() => 0,
        Some(_) => 1,
        None => 3,
    }
}

fn pick_jar(dir: &Path) -> JarPick {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return JarPick::default();
    };
    let mut jars: Vec<JarName> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|name| JarName::parse(&name).ok())
        .collect();
    jars.sort_by_key(|jar| (jar.as_str() != "server.jar", jar.as_str().to_owned()));
    let mut found: Vec<JarPick> = jars
        .into_iter()
        .map(|jar| {
            let (loader, minecraft, loader_version) =
                inspect_jar(&dir.join(jar.as_str()), jar.as_str());
            JarPick {
                jar: Some(jar),
                loader,
                minecraft,
                loader_version,
            }
        })
        .collect();
    // Sắp xếp ổn định: cùng hạng thì giữ thứ tự `server.jar` trước, rồi theo tên.
    found.sort_by_key(|pick| jar_rank(pick.loader));
    // Jar khởi động của Fabric/Quilt không ghi bản Minecraft; lấy từ `server.jar` nằm cạnh.
    let minecraft = found.iter().find_map(|pick| pick.minecraft.clone());
    let mut best = found.into_iter().next().unwrap_or_default();
    best.minecraft = best.minecraft.or(minecraft);
    best
}

/// Paper và Purpur đóng gói bằng paperclip; jar của Mojang từ 1.18 có `net/minecraft/bundler/`,
/// bản cũ hơn có thẳng `net/minecraft/server/`. Fabric/Quilt nhận qua `META-INF/MANIFEST.MF`.
fn inspect_jar(
    path: &Path,
    name: &str,
) -> (Option<LoaderKind>, Option<McVersion>, Option<LoaderVersion>) {
    let Ok(file) = File::open(path) else {
        return (None, None, None);
    };
    let Ok(mut zip) = zip::ZipArchive::new(file) else {
        return (None, None, None);
    };
    let names: Vec<String> = zip
        .file_names()
        .filter_map(Result::ok)
        .map(|name| name.into_owned())
        .collect();
    let has = |prefix: &str| names.iter().any(|n| n.starts_with(prefix));
    let main_class = read_zip_text(&mut zip, "META-INF/MANIFEST.MF")
        .as_deref()
        .and_then(manifest_main_class);
    let loader = detect_jar_loader(name, &has, main_class.as_deref());
    let (mc_from_name, lv_from_name) = match loader {
        Some(LoaderKind::Forge) => parse_forge_jar_name(name),
        _ => parse_fabric_jar_name(name),
    };
    let lv = lv_from_name.or_else(|| loader.and_then(|l| loader_version_in_zip(&names, l)));
    let version = read_zip_text(&mut zip, "version.json")
        .and_then(|text| serde_json::from_str::<VersionJson>(&text).ok())
        .and_then(|json| McVersion::parse(&json.id).ok())
        .or(mc_from_name);
    (loader, version, lv)
}

fn detect_jar_loader(
    name: &str,
    has: &dyn Fn(&str) -> bool,
    main_class: Option<&str>,
) -> Option<LoaderKind> {
    if main_class.is_some_and(|m| m.contains("net.fabricmc.")) {
        return Some(LoaderKind::Fabric);
    }
    if main_class.is_some_and(|m| m.contains("org.quiltmc.")) {
        return Some(LoaderKind::Quilt);
    }
    // Forge trước 1.17 chạy thẳng `forge-<mc>-<bản>.jar` do installer tạo, không có args file.
    if main_class.is_some_and(|m| m.contains("net.minecraftforge."))
        || (name.starts_with("forge-") && has("net/minecraftforge/"))
    {
        return Some(LoaderKind::Forge);
    }
    if has("io/papermc/paperclip/") {
        if name.to_ascii_lowercase().contains("purpur") {
            Some(LoaderKind::Purpur)
        } else {
            Some(LoaderKind::Paper)
        }
    } else if has("net/minecraft/bundler/") || has("net/minecraft/server/") {
        Some(LoaderKind::Vanilla)
    } else {
        None
    }
}

fn read_zip_text(zip: &mut zip::ZipArchive<File>, entry_name: &str) -> Option<String> {
    let entry = zip.by_name(entry_name).ok()?;
    let mut text = String::new();
    entry.take(SMALL_JSON).read_to_string(&mut text).ok()?;
    Some(text)
}

fn manifest_main_class(manifest: &str) -> Option<String> {
    manifest
        .lines()
        .find_map(|line| line.strip_prefix("Main-Class:"))
        .map(|s| s.trim().to_owned())
}

fn loader_version_in_zip(names: &[String], loader: LoaderKind) -> Option<LoaderVersion> {
    let prefix = match loader {
        LoaderKind::Fabric => "META-INF/jars/fabric-loader-",
        LoaderKind::Quilt => "META-INF/jars/quilt-loader-",
        _ => return None,
    };
    names.iter().find_map(|n| {
        let rest = n.strip_prefix(prefix)?.strip_suffix(".jar")?;
        LoaderVersion::parse(rest).ok()
    })
}

/// Tên jar của Fabric từ trang tải đôi khi có dạng `fabric-server-mc.<mc>-loader.<loader>-launcher.<ver>.jar`.
fn parse_fabric_jar_name(name: &str) -> (Option<McVersion>, Option<LoaderVersion>) {
    let Some(rest) = name.strip_prefix("fabric-server-mc.") else {
        return (None, None);
    };
    let Some((mc_s, tail)) = rest.split_once("-loader.") else {
        return (None, None);
    };
    let lv_s = tail
        .split_once("-launcher.")
        .map(|(lv, _)| lv)
        .unwrap_or(tail.trim_end_matches(".jar"));
    (McVersion::parse(mc_s).ok(), LoaderVersion::parse(lv_s).ok())
}

/// Jar Forge cũ: `forge-<mc>-<bản>.jar`, bản rất cũ thêm hậu tố `-universal`.
fn parse_forge_jar_name(name: &str) -> (Option<McVersion>, Option<LoaderVersion>) {
    let Some(rest) = name
        .strip_prefix("forge-")
        .and_then(|rest| rest.strip_suffix(".jar"))
    else {
        return (None, None);
    };
    let rest = rest.strip_suffix("-universal").unwrap_or(rest);
    match rest.split_once('-') {
        Some((mc, forge)) => (McVersion::parse(mc).ok(), LoaderVersion::parse(forge).ok()),
        None => (None, None),
    }
}

fn detect_args_file(
    dir: &Path,
) -> Option<(LoaderKind, Option<LoaderVersion>, Option<McVersion>, Launch)> {
    if let Some((ver, launch)) = scan_args_dir(dir, "libraries/net/neoforged/neoforge") {
        let mc = neoforge_minecraft(&ver).and_then(|s| McVersion::parse(&s).ok());
        let lv = LoaderVersion::parse(&ver).ok();
        return Some((LoaderKind::NeoForge, lv, mc, launch));
    }
    if let Some((ver, launch)) = scan_args_dir(dir, "libraries/net/minecraftforge/forge") {
        let (mc, lv) = match ver.split_once('-') {
            Some((mc_s, forge_s)) => (
                McVersion::parse(mc_s).ok(),
                LoaderVersion::parse(forge_s).ok(),
            ),
            None => (None, LoaderVersion::parse(&ver).ok()),
        };
        return Some((LoaderKind::Forge, lv, mc, launch));
    }
    None
}

fn scan_args_dir(dir: &Path, sub: &str) -> Option<(String, Launch)> {
    newest_first(&dir.join(sub)).into_iter().find_map(|ver| {
        let folder = dir.join(sub).join(&ver);
        let has_args =
            folder.join("unix_args.txt").is_file() || folder.join("win_args.txt").is_file();
        let path = ArgsFilePath::parse(&format!("{sub}/{ver}/{{os}}_args.txt")).ok()?;
        has_args.then_some((ver, Launch::ArgsFile(path)))
    })
}

fn detect_mc_from_dirs(dir: &Path) -> Option<McVersion> {
    newest_first(&dir.join("versions"))
        .into_iter()
        .find_map(|d| McVersion::parse(&d).ok())
}

fn detect_loader_from_dirs(dir: &Path, loader: LoaderKind) -> Option<LoaderVersion> {
    let sub = match loader {
        LoaderKind::Fabric => "libraries/net/fabricmc/fabric-loader",
        LoaderKind::Quilt => "libraries/org/quiltmc/quilt-loader",
        _ => return None,
    };
    newest_first(&dir.join(sub))
        .into_iter()
        .find_map(|d| LoaderVersion::parse(&d).ok())
}

/// Tên các thư mục con, bản mới nhất trước. Nâng cấp tại chỗ thường để lại thư mục bản cũ.
fn newest_first(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|k| k.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort_by_key(|name| std::cmp::Reverse(version_key(name)));
    names
}

/// Khóa so phiên bản theo từng số: `21.1.10` mới hơn `21.1.9`, `1.20` mới hơn `1.9`.
/// Phần không phải số (`beta`, `pre1`) xếp sau mọi số ở cùng vị trí.
fn version_key(name: &str) -> Vec<(u64, String)> {
    name.split(['.', '-', '+'])
        .map(|part| match part.parse::<u64>() {
            Ok(n) => (n + 1, String::new()),
            Err(_) => (0, part.to_owned()),
        })
        .collect()
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod tests;
