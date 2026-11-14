//! Đọc gói modpack `.mrpack` của Modrinth và giải nén `overrides/` + `server-overrides/`
//! qua `SafeRoot` với giới hạn chống zip bomb (spec P3, mục 3.5, 3.7, 5.4, 8).

use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::Path;

use mh_core::{LoaderKind, LoaderVersion, McVersion, MrpackIndex, MrpackRuntime};
use mh_net::{Checksum, HostAllowlist, hosts};
use mh_platform::SafeRoot;
use zip::ZipArchive;
use zip::result::ZipError;

mod archive;

use crate::ProviderError;
use archive::{
    add_within_limit, extract_overrides, is_symlink_or_special, is_unix_dir, map_read_io,
    validate_archive_path, zip_err,
};

pub const MAX_INDEX_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const MAX_FILES: usize = 20_000;
const COPY_BUF_BYTES: usize = 64 * 1024;
const INDEX_NAME: &str = "modrinth.index.json";

/// Giới hạn chống zip bomb khi đọc modpack; tách thành tham số để test dùng ngưỡng nhỏ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackLimits {
    pub max_index_bytes: u64,
    pub max_total_bytes: u64,
    pub max_files: usize,
}

impl Default for PackLimits {
    fn default() -> Self {
        Self {
            max_index_bytes: MAX_INDEX_BYTES,
            max_total_bytes: MAX_TOTAL_BYTES,
            max_files: MAX_FILES,
        }
    }
}

/// Một file trong modpack cần tải về máy chủ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackFile {
    pub path: String,
    pub urls: Vec<String>,
    pub checksum: Checksum,
    pub sha1: String,
    pub size: u64,
    pub hosts: HostAllowlist,
}

/// File không thể tự tải qua API (dành cho CurseForge ở PR-29; `.mrpack` luôn để trống).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedPackFile {
    pub path: String,
    pub project_id: u64,
    pub file_id: u64,
    pub sha1: String,
    pub page_url: String,
}

/// Kế hoạch cài modpack sau khi đã giải nén phần overrides vào thư mục đích.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackPlan {
    pub name: String,
    pub version_id: String,
    pub loader: LoaderKind,
    pub minecraft: McVersion,
    pub loader_version: Option<LoaderVersion>,
    pub files: Vec<PackFile>,
    pub skipped_client: Vec<String>,
    pub blocked: Vec<BlockedPackFile>,
}

struct OverrideEntry {
    index: usize,
    rel: String,
    is_dir: bool,
}

/// Lập kế hoạch cài `.mrpack` (chỉ đọc index và quét metadata overrides, không ghi đĩa).
pub fn plan_mrpack(archive_path: &Path) -> Result<PackPlan, ProviderError> {
    plan_mrpack_with(archive_path, hosts::MODRINTH, PackLimits::default())
}

/// Lập kế hoạch cài `.mrpack` với danh sách host và giới hạn chống zip bomb cho trước.
pub fn plan_mrpack_with(
    archive_path: &Path,
    hosts: HostAllowlist,
    limits: PackLimits,
) -> Result<PackPlan, ProviderError> {
    let mut archive = open_mrpack(archive_path, limits)?;
    let (index, runtime) = read_index(&mut archive, archive_path, limits)?;
    let (files, skipped_client, index_bytes, index_count) =
        plan_index_files(index.files, hosts, limits)?;
    // Quét trước để bắt zip bomb / tên file không hợp lệ trước khi tải loader hay file.
    let _ = scan_overrides(&mut archive, archive_path, limits, index_count, index_bytes)?;

    Ok(PackPlan {
        name: index.name,
        version_id: index.version_id,
        loader: runtime.loader,
        minecraft: runtime.minecraft,
        loader_version: runtime.loader_version,
        files,
        skipped_client,
        blocked: Vec::new(),
    })
}

/// Giải nén `overrides/` rồi `server-overrides/` vào `dest_dir` qua `SafeRoot` với giới hạn mặc định.
pub fn extract_mrpack_overrides(archive_path: &Path, dest_dir: &Path) -> Result<(), ProviderError> {
    extract_mrpack_overrides_with(archive_path, dest_dir, PackLimits::default())
}

/// Giải nén `overrides/` rồi `server-overrides/` vào `dest_dir` sau khi tải xong index (spec P3, mục 3.4).
pub fn extract_mrpack_overrides_with(
    archive_path: &Path,
    dest_dir: &Path,
    limits: PackLimits,
) -> Result<(), ProviderError> {
    let mut archive = open_mrpack(archive_path, limits)?;
    let (index, _) = read_index(&mut archive, archive_path, limits)?;
    let index_count = index.files.len();
    if index_count > limits.max_files {
        return Err(ProviderError::modpack_invalid(
            "số lượng file trong modpack vượt giới hạn",
        ));
    }
    let index_bytes = index
        .files
        .iter()
        .filter(|f| !f.is_client_only())
        .try_fold(0u64, |sum, f| {
            add_within_limit(
                sum,
                f.file_size,
                limits.max_total_bytes,
                "tổng kích thước modpack vượt giới hạn",
            )
        })?;
    let (overrides, server_overrides) =
        scan_overrides(&mut archive, archive_path, limits, index_count, index_bytes)?;

    fs::create_dir_all(dest_dir).map_err(|e| ProviderError::io(dest_dir, e))?;
    let root = SafeRoot::new(dest_dir)?;
    extract_overrides(
        &mut archive,
        archive_path,
        &root,
        overrides,
        server_overrides,
        limits,
        index_bytes,
    )
}

/// Đọc `.mrpack` với danh sách host Modrinth và giới hạn chống zip bomb mặc định.
pub fn read_mrpack(archive_path: &Path, dest_dir: &Path) -> Result<PackPlan, ProviderError> {
    read_mrpack_with(
        archive_path,
        dest_dir,
        hosts::MODRINTH,
        PackLimits::default(),
    )
}

/// Đọc `.mrpack`, lập kế hoạch và giải nén `overrides/` rồi `server-overrides/` vào `dest_dir`.
pub fn read_mrpack_with(
    archive_path: &Path,
    dest_dir: &Path,
    hosts: HostAllowlist,
    limits: PackLimits,
) -> Result<PackPlan, ProviderError> {
    let plan = plan_mrpack_with(archive_path, hosts, limits)?;
    extract_mrpack_overrides_with(archive_path, dest_dir, limits)?;
    Ok(plan)
}

fn open_mrpack(
    path: &Path,
    limits: PackLimits,
) -> Result<ZipArchive<BufReader<File>>, ProviderError> {
    let file = File::open(path).map_err(|e| ProviderError::io(path, e))?;
    let archive = ZipArchive::new(BufReader::new(file)).map_err(|e| zip_err(path, e))?;
    // Chặn archive chứa hàng trăm nghìn mục thư mục hoặc mục bị bỏ qua để tránh treo vòng lặp.
    let entry_cap = limits.max_files.saturating_mul(4).max(1024);
    if archive.len() > entry_cap {
        return Err(ProviderError::modpack_invalid(
            "modpack chứa quá nhiều mục trong zip",
        ));
    }
    Ok(archive)
}

fn read_index(
    archive: &mut ZipArchive<BufReader<File>>,
    archive_path: &Path,
    limits: PackLimits,
) -> Result<(MrpackIndex, MrpackRuntime), ProviderError> {
    let entry = match archive.by_name(INDEX_NAME) {
        Ok(entry) => entry,
        Err(ZipError::FileNotFound) => {
            return Err(ProviderError::modpack_invalid("thiếu modrinth.index.json"));
        }
        Err(e) => return Err(zip_err(archive_path, e)),
    };
    if entry.is_dir() || is_unix_dir(&entry) || is_symlink_or_special(&entry) {
        return Err(ProviderError::modpack_invalid(
            "modrinth.index.json không phải file thường",
        ));
    }
    if entry.size() > limits.max_index_bytes {
        return Err(ProviderError::modpack_invalid(
            "modrinth.index.json vượt quá giới hạn kích thước",
        ));
    }
    let mut buf = Vec::new();
    entry
        .take(limits.max_index_bytes.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(|e| map_read_io(archive_path, e))?;
    if buf.len() as u64 > limits.max_index_bytes {
        return Err(ProviderError::modpack_invalid(
            "modrinth.index.json vượt quá giới hạn kích thước",
        ));
    }
    let text = String::from_utf8(buf)
        .map_err(|_| ProviderError::modpack_invalid("modrinth.index.json không phải UTF-8"))?;
    let index =
        MrpackIndex::from_json(&text).map_err(|e| ProviderError::modpack_invalid(e.to_string()))?;
    let runtime = index
        .resolve_runtime()
        .map_err(|e| ProviderError::modpack_invalid(e.to_string()))?;
    Ok((index, runtime))
}

fn plan_index_files(
    entries: Vec<mh_core::MrpackFile>,
    hosts: HostAllowlist,
    limits: PackLimits,
) -> Result<(Vec<PackFile>, Vec<String>, u64, usize), ProviderError> {
    let index_count = entries.len();
    if index_count > limits.max_files {
        return Err(ProviderError::modpack_invalid(
            "số lượng file trong modpack vượt giới hạn",
        ));
    }
    let mut files = Vec::new();
    let mut skipped_client = Vec::new();
    let mut planned_bytes = 0u64;

    for item in entries {
        validate_archive_path(&item.path)?;
        if item.is_client_only() {
            skipped_client.push(item.path);
            continue;
        }
        if item.downloads.is_empty() {
            return Err(ProviderError::modpack_invalid(format!(
                "file `{}` không có URL tải",
                item.path
            )));
        }
        for url in &item.downloads {
            hosts.check_str(url).map_err(|e| {
                tracing::warn!(path = item.path, url, error = %e, "URL trong .mrpack ngoài danh sách host");
                ProviderError::Net(e)
            })?;
        }
        let sha1 = Checksum::sha1(&item.hashes.sha1)
            .ok_or_else(|| ProviderError::modpack_invalid("sha1 trong index không hợp lệ"))?;
        let checksum = Checksum::sha512(&item.hashes.sha512)
            .ok_or_else(|| ProviderError::modpack_invalid("sha512 trong index không hợp lệ"))?;
        planned_bytes = add_within_limit(
            planned_bytes,
            item.file_size,
            limits.max_total_bytes,
            "tổng kích thước modpack vượt giới hạn",
        )?;
        files.push(PackFile {
            path: item.path,
            urls: item.downloads,
            checksum,
            sha1: sha1.hex().to_owned(),
            size: item.file_size,
            hosts,
        });
    }
    Ok((files, skipped_client, planned_bytes, index_count))
}

fn scan_overrides(
    archive: &mut ZipArchive<BufReader<File>>,
    archive_path: &Path,
    limits: PackLimits,
    mut total_files: usize,
    mut declared_bytes: u64,
) -> Result<(Vec<OverrideEntry>, Vec<OverrideEntry>), ProviderError> {
    let mut overrides = Vec::new();
    let mut server_overrides = Vec::new();

    for index in 0..archive.len() {
        let Some((is_server, item, size)) = inspect_override_entry(archive, index, archive_path)?
        else {
            continue;
        };
        if !item.is_dir {
            total_files = total_files
                .checked_add(1)
                .filter(|&n| n <= limits.max_files)
                .ok_or_else(|| {
                    ProviderError::modpack_invalid("số lượng file trong modpack vượt giới hạn")
                })?;
            declared_bytes = add_within_limit(
                declared_bytes,
                size,
                limits.max_total_bytes,
                "tổng kích thước modpack vượt giới hạn",
            )?;
        }
        if is_server {
            server_overrides.push(item);
        } else {
            overrides.push(item);
        }
    }
    Ok((overrides, server_overrides))
}

fn inspect_override_entry(
    archive: &mut ZipArchive<BufReader<File>>,
    index: usize,
    archive_path: &Path,
) -> Result<Option<(bool, OverrideEntry, u64)>, ProviderError> {
    let (name, is_dir, skip_special, size) = {
        let entry = archive
            .by_index_raw(index)
            .map_err(|e| zip_err(archive_path, e))?;
        let name = entry
            .name()
            .map_err(|e| zip_err(archive_path, e))?
            .into_owned();
        let is_dir = entry.is_dir() || is_unix_dir(&entry);
        (name, is_dir, is_symlink_or_special(&entry), entry.size())
    };
    if name == INDEX_NAME {
        return Ok(None);
    }
    let raw_check = if is_dir {
        name.strip_suffix('/').unwrap_or(&name)
    } else {
        &name
    };
    validate_archive_path(raw_check)?;
    if skip_special {
        tracing::debug!(%name, "bỏ qua symlink hoặc file đặc biệt trong modpack");
        return Ok(None);
    }
    let Some((is_server, rel)) = name
        .strip_prefix("overrides/")
        .map(|rel| (false, rel))
        .or_else(|| {
            name.strip_prefix("server-overrides/")
                .map(|rel| (true, rel))
        })
    else {
        // Bỏ qua `client-overrides/` và các thư mục/file lạ ở gốc zip.
        return Ok(None);
    };
    let rel = if is_dir {
        rel.strip_suffix('/').unwrap_or(rel)
    } else {
        rel
    };
    if rel.is_empty() {
        return Ok(None);
    }
    Ok(Some((
        is_server,
        OverrideEntry {
            index,
            rel: rel.to_owned(),
            is_dir,
        },
        size,
    )))
}
