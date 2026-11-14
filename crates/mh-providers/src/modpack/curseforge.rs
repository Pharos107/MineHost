//! Đọc modpack CurseForge (`.zip` thường + server pack), giải nén `overrides` qua `SafeRoot`
//! với cùng giới hạn chống zip bomb như `.mrpack` (spec P3, mục 3.5, 3.6, 5.4, 6.2).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use mh_core::{CfManifest, CfManifestFile, CfRuntime};

use super::archive::{
    RawZipEntry, add_within_limit, count_within_limits, extract_entries, open_pack_archive,
    read_entry_meta, read_json_entry, scan_prefixed_layers, validate_archive_path,
};
use super::{BlockedPackFile, OverrideEntry, PackFile, PackLimits, PackPlan};
use crate::curseforge::ModInfo;
use crate::{CurseForge, ProviderError};

const CF_MANIFEST_NAME: &str = "manifest.json";
const SOURCE: &str = "CurseForge";

/// Các thư mục cấp gốc chuẩn của server Minecraft — nếu toàn bộ server pack chỉ có một thư mục
/// này thì KHÔNG coi là thư mục lồng ngoài cùng.
const STANDARD_SERVER_DIRS: &[&str] = &[
    "mods",
    "config",
    "defaultconfigs",
    "libraries",
    "kubejs",
    "scripts",
    "world",
    "plugins",
    "resourcepacks",
    "shaderpacks",
    "datapacks",
    "local",
];

/// Đọc và kiểm tra `manifest.json` + metadata thư mục `overrides` trong `.zip` CurseForge (không ghi đĩa).
pub fn inspect_curseforge_pack(archive_path: &Path) -> Result<CfManifest, ProviderError> {
    inspect_curseforge_pack_with(archive_path, PackLimits::default())
}

/// Như [`inspect_curseforge_pack`] nhưng nhận giới hạn chống zip bomb tùy chỉnh (dùng trong test).
pub fn inspect_curseforge_pack_with(
    archive_path: &Path,
    limits: PackLimits,
) -> Result<CfManifest, ProviderError> {
    let (manifest, _, _, _) = scan_cf_manifest_and_overrides(archive_path, limits)?;
    Ok(manifest)
}

/// Giải quyết danh sách `(projectID, fileID)` của `manifest.json` qua API CurseForge thành file tải được và file chờ tay.
pub async fn resolve_curseforge_files(
    cf: &CurseForge,
    manifest_files: &[CfManifestFile],
) -> Result<(Vec<PackFile>, Vec<BlockedPackFile>), ProviderError> {
    if manifest_files.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let file_ids: Vec<u64> = manifest_files.iter().map(|f| f.file_id).collect();
    let fetched = cf.files_by_id(&file_ids).await?;
    let mut by_file: HashMap<u64, crate::curseforge::ModFile> =
        fetched.into_iter().map(|f| (f.id, f)).collect();

    let mod_ids: Vec<u64> = manifest_files
        .iter()
        .map(|f| f.project_id)
        .chain(by_file.values().map(|f| f.mod_id))
        .collect();
    let mods: HashMap<u64, ModInfo> = cf
        .mods_by_id(&mod_ids)
        .await?
        .into_iter()
        .map(|m| (m.id, m))
        .collect();

    let mut seen = HashSet::new();
    let mut pack_files = Vec::new();
    let mut blocked = Vec::new();
    for entry in manifest_files {
        if !seen.insert(entry.file_id) {
            continue;
        }
        let Some(mut file) = by_file.remove(&entry.file_id) else {
            if entry.required {
                return Err(ProviderError::ContentNotFound {
                    source_name: SOURCE,
                    id: entry.file_id.to_string(),
                });
            }
            continue;
        };
        if let Some(info) = mods
            .get(&file.mod_id)
            .or_else(|| mods.get(&entry.project_id))
        {
            file.apply_mod(info);
        }
        match classify_cf_file(file, cf.hosts())? {
            Ok(pf) => pack_files.push(pf),
            Err(bf) => blocked.push(bf),
        }
    }
    Ok((pack_files, blocked))
}

fn classify_cf_file(
    file: crate::curseforge::ModFile,
    hosts: mh_net::HostAllowlist,
) -> Result<Result<PackFile, BlockedPackFile>, ProviderError> {
    if file.file_name.contains('/') {
        return Err(ProviderError::modpack_invalid(format!(
            "tên file CurseForge `{}` không hợp lệ",
            file.file_name
        )));
    }
    validate_archive_path(&file.file_name)?;
    let path = format!("mods/{}", file.file_name);
    if let Some(url) = file.api_download_url().map(str::to_owned) {
        let Some(checksum) = file.checksum() else {
            return Err(ProviderError::invalid(
                SOURCE,
                format!("file `{}` thiếu hash SHA-1", file.file_name),
            ));
        };
        return Ok(Ok(PackFile {
            path,
            urls: vec![url],
            checksum,
            sha1: file.sha1.unwrap_or_default(),
            size: file.file_length,
            hosts,
            cf_ids: Some((file.mod_id, file.id)),
        }));
    }
    let page_url = file.download_page_url.unwrap_or_else(|| {
        format!(
            "https://www.curseforge.com/projects/{}/files/{}",
            file.mod_id, file.id
        )
    });
    Ok(Err(BlockedPackFile {
        path,
        project_id: file.mod_id,
        file_id: file.id,
        sha1: file.sha1.unwrap_or_default(),
        page_url,
    }))
}

/// Lập kế hoạch cài pack `.zip` CurseForge (đọc manifest, quét overrides, gọi API lấy danh sách mod).
pub async fn plan_curseforge_pack(
    archive_path: &Path,
    cf: &CurseForge,
) -> Result<PackPlan, ProviderError> {
    plan_curseforge_pack_with(archive_path, cf, PackLimits::default()).await
}

/// Như [`plan_curseforge_pack`] nhưng nhận `PackLimits` tùy chỉnh.
pub async fn plan_curseforge_pack_with(
    archive_path: &Path,
    cf: &CurseForge,
    limits: PackLimits,
) -> Result<PackPlan, ProviderError> {
    let (manifest, runtime, override_files, override_bytes) =
        scan_cf_manifest_and_overrides(archive_path, limits)?;
    let (files, blocked) = resolve_curseforge_files(cf, &manifest.files).await?;
    check_combined_limits(&files, override_files, override_bytes, limits)?;
    Ok(PackPlan {
        name: manifest.name,
        version_id: manifest.version,
        loader: runtime.loader,
        minecraft: runtime.minecraft,
        loader_version: runtime.loader_version,
        files,
        skipped_client: Vec::new(),
        blocked,
    })
}

/// Giải nén thư mục `overrides` (như khai báo trong `manifest.json`) vào `dest_dir` qua `SafeRoot`.
pub fn extract_curseforge_overrides(
    archive_path: &Path,
    dest_dir: &Path,
) -> Result<(), ProviderError> {
    extract_curseforge_overrides_with(archive_path, dest_dir, PackLimits::default())
}

/// Giải nén thư mục `overrides` của pack CurseForge với giới hạn chống zip bomb cho trước.
pub fn extract_curseforge_overrides_with(
    archive_path: &Path,
    dest_dir: &Path,
    limits: PackLimits,
) -> Result<(), ProviderError> {
    let mut archive = open_pack_archive(archive_path, limits)?;
    let (manifest, _) = read_cf_manifest(&mut archive, archive_path, limits)?;
    let prefix = format!("{}/", manifest.overrides);
    let mut total_files = 0usize;
    let mut declared_bytes = 0u64;
    let mut layers = scan_prefixed_layers(
        &mut archive,
        archive_path,
        Some(CF_MANIFEST_NAME),
        &[prefix.as_str()],
        limits,
        &mut total_files,
        &mut declared_bytes,
    )?;
    let entries = layers.pop().unwrap_or_default();
    extract_entries(&mut archive, archive_path, dest_dir, entries, limits, 0)
}

/// Giải nén server pack `.zip` CurseForge vào `dest_dir`, trả danh sách entry khởi chạy/installer đã bỏ qua.
pub fn extract_curseforge_server_pack(
    archive_path: &Path,
    dest_dir: &Path,
) -> Result<Vec<String>, ProviderError> {
    extract_curseforge_server_pack_with(archive_path, dest_dir, PackLimits::default())
}

/// Giải nén server pack `.zip` CurseForge với giới hạn chống zip bomb; tự bỏ thư mục gốc lồng một tầng.
pub fn extract_curseforge_server_pack_with(
    archive_path: &Path,
    dest_dir: &Path,
    limits: PackLimits,
) -> Result<Vec<String>, ProviderError> {
    let mut archive = open_pack_archive(archive_path, limits)?;
    let mut raw_entries = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let meta = read_entry_meta(&mut archive, index, archive_path)?;
        if meta.skip_special {
            tracing::debug!(name = %meta.name, "bỏ qua symlink hoặc file đặc biệt trong server pack");
            continue;
        }
        raw_entries.push(meta);
    }

    let nested_root = detect_nested_root(&raw_entries);
    let mut total_files = 0usize;
    let mut declared_bytes = 0u64;
    let mut kept = Vec::new();
    let mut skipped = Vec::new();

    for meta in raw_entries {
        let Some(rel) = strip_nested_root(&meta, nested_root.as_deref()) else {
            continue;
        };
        if let Some(reason) = skipped_server_pack_reason(rel) {
            tracing::debug!(
                path = rel,
                reason,
                "bỏ qua file khởi chạy/installer của server pack"
            );
            skipped.push(rel.to_owned());
            continue;
        }
        count_within_limits(
            meta.is_dir,
            meta.size,
            limits,
            &mut total_files,
            &mut declared_bytes,
        )?;
        kept.push(OverrideEntry {
            index: meta.index,
            rel: rel.to_owned(),
            is_dir: meta.is_dir,
        });
    }

    extract_entries(&mut archive, archive_path, dest_dir, kept, limits, 0)?;
    Ok(skipped)
}

pub(crate) fn check_combined_limits(
    files: &[PackFile],
    override_files: usize,
    override_bytes: u64,
    limits: PackLimits,
) -> Result<(), ProviderError> {
    let total_count = files
        .len()
        .checked_add(override_files)
        .filter(|&n| n <= limits.max_files)
        .ok_or_else(|| {
            ProviderError::modpack_invalid("số lượng file trong modpack vượt giới hạn")
        })?;
    let _ = total_count;
    let files_bytes = files.iter().try_fold(0u64, |sum, f| {
        add_within_limit(
            sum,
            f.size,
            limits.max_total_bytes,
            "tổng kích thước modpack vượt giới hạn",
        )
    })?;
    add_within_limit(
        files_bytes,
        override_bytes,
        limits.max_total_bytes,
        "tổng kích thước modpack vượt giới hạn",
    )?;
    Ok(())
}

fn scan_cf_manifest_and_overrides(
    archive_path: &Path,
    limits: PackLimits,
) -> Result<(CfManifest, CfRuntime, usize, u64), ProviderError> {
    let mut archive = open_pack_archive(archive_path, limits)?;
    let (manifest, runtime) = read_cf_manifest(&mut archive, archive_path, limits)?;
    let prefix = format!("{}/", manifest.overrides);
    let mut total_files = 0usize;
    let mut declared_bytes = 0u64;
    let _ = scan_prefixed_layers(
        &mut archive,
        archive_path,
        Some(CF_MANIFEST_NAME),
        &[prefix.as_str()],
        limits,
        &mut total_files,
        &mut declared_bytes,
    )?;
    Ok((manifest, runtime, total_files, declared_bytes))
}

fn read_cf_manifest(
    archive: &mut zip::ZipArchive<std::io::BufReader<std::fs::File>>,
    archive_path: &Path,
    limits: PackLimits,
) -> Result<(CfManifest, CfRuntime), ProviderError> {
    let text = read_json_entry(
        archive,
        archive_path,
        CF_MANIFEST_NAME,
        limits.max_index_bytes,
    )?;
    let manifest =
        CfManifest::from_json(&text).map_err(|e| ProviderError::modpack_invalid(e.to_string()))?;
    validate_archive_path(&manifest.overrides)?;
    let runtime = manifest
        .resolve_runtime()
        .map_err(|e| ProviderError::modpack_invalid(e.to_string()))?;
    Ok((manifest, runtime))
}

/// Phát hiện trường hợp mọi entry trong server pack nằm lồng một tầng dưới cùng một thư mục gốc
/// (không tính các thư mục chuẩn như `mods/`, `config/` nếu zip vốn đã ở dạng phẳng).
fn detect_nested_root(entries: &[RawZipEntry]) -> Option<String> {
    let mut root: Option<&str> = None;
    let mut has_child = false;
    for meta in entries {
        let trimmed = if meta.is_dir {
            meta.name.strip_suffix('/').unwrap_or(&meta.name)
        } else {
            &meta.name
        };
        if trimmed.is_empty() {
            continue;
        }
        let (first, rest) = match trimmed.split_once('/') {
            Some((first, rest)) => {
                if !rest.is_empty() {
                    has_child = true;
                }
                (first, rest)
            }
            None if meta.is_dir => (trimmed, ""),
            // File nằm trực tiếp ở gốc zip => không có thư mục bao ngoài.
            None => return None,
        };
        let _ = rest;
        match root {
            None => root = Some(first),
            Some(prev) if prev == first => {}
            Some(_) => return None,
        }
    }
    let root = root?;
    if !has_child {
        return None;
    }
    let lower = root.to_ascii_lowercase();
    if STANDARD_SERVER_DIRS.contains(&lower.as_str()) {
        return None;
    }
    Some(root.to_owned())
}

fn strip_nested_root<'a>(meta: &'a RawZipEntry, nested_root: Option<&str>) -> Option<&'a str> {
    let trimmed = if meta.is_dir {
        meta.name.strip_suffix('/').unwrap_or(&meta.name)
    } else {
        meta.name.as_str()
    };
    let rel = match nested_root {
        Some(root) => trimmed.strip_prefix(root)?.strip_prefix('/')?,
        None => trimmed,
    };
    (!rel.is_empty()).then_some(rel)
}

/// Danh sách file khởi chạy/installer trong server pack CurseForge bị bỏ qua cùng lý do:
/// - `libraries/`: MineHost tự chạy installer sạch theo OS hiện tại (`setup_loader_server`), tránh
///   thư viện/args sai hệ điều hành hoặc xung đột.
/// - `*.sh`, `*.bat`, `*.cmd`: script khởi chạy ngoài; MineHost điều khiển JVM trực tiếp qua `mh-runtime`.
/// - Jar installer hoặc jar server của loader ở thư mục gốc (`!rel.contains('/')`): MineHost tự tải
///   và cài loader; chỉ kiểm ở cấp gốc để không chạm tới mod nằm trong `mods/`.
pub fn skipped_server_pack_reason(rel: &str) -> Option<&'static str> {
    let lower = rel.to_ascii_lowercase();
    if lower == "libraries" || lower.starts_with("libraries/") {
        return Some("thư viện loader do MineHost tự cài");
    }
    if [".sh", ".bat", ".cmd"]
        .iter()
        .any(|ext| lower.ends_with(ext))
    {
        return Some("script khởi chạy ngoài không dùng với MineHost");
    }
    if !lower.contains('/') && lower.ends_with(".jar") && is_loader_or_installer_jar(&lower) {
        return Some("jar installer hoặc jar server của loader do MineHost tự cài");
    }
    None
}

fn is_loader_or_installer_jar(lower_name: &str) -> bool {
    lower_name == "server.jar"
        || lower_name.contains("installer")
        || lower_name.starts_with("forge-")
        || lower_name.starts_with("neoforge-")
        || lower_name.starts_with("fabric-")
        || lower_name.starts_with("quilt-")
        || lower_name.starts_with("minecraft_server")
        || lower_name.ends_with("-shim.jar")
        || lower_name.ends_with("-universal.jar")
}
