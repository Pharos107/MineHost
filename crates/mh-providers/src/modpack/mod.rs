//! Đọc gói modpack `.mrpack` (Modrinth) và `.zip` (CurseForge), giải nén overrides
//! qua `SafeRoot` với giới hạn chống zip bomb (spec P3, mục 3.5, 3.6, 3.7, 5.4, 8).

use std::path::Path;

use mh_core::{LoaderKind, LoaderVersion, McVersion, MrpackIndex, MrpackRuntime};
use mh_net::{Checksum, HostAllowlist, hosts};

mod archive;
mod curseforge;

use crate::ProviderError;
use archive::{
    add_within_limit, extract_entries, open_pack_archive, read_json_entry, scan_prefixed_layers,
    validate_archive_path,
};

pub use curseforge::{
    extract_curseforge_overrides, extract_curseforge_overrides_after,
    extract_curseforge_overrides_with, extract_curseforge_server_pack,
    extract_curseforge_server_pack_with, inspect_curseforge_pack, inspect_curseforge_pack_with,
    plan_curseforge_pack, plan_curseforge_pack_with, resolve_curseforge_files,
    server_pack_has_mods, skipped_server_pack_reason,
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
    /// Cặp `(project_id, file_id)` khi đọc từ manifest CurseForge, để ghi `content.toml`.
    pub cf_ids: Option<(u64, u64)>,
}

/// File không thể tự tải qua API (CurseForge cấm phân phối hoặc thiếu `downloadUrl`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedPackFile {
    pub path: String,
    pub project_id: u64,
    pub file_id: u64,
    pub sha1: String,
    pub page_url: String,
}

/// Kế hoạch cài modpack sau khi đọc index/manifest (chưa tải file hay giải nén overrides).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackPlan {
    pub name: String,
    pub version_id: String,
    pub loader: LoaderKind,
    pub minecraft: McVersion,
    pub loader_version: Option<LoaderVersion>,
    pub files: Vec<PackFile>,
    pub skipped_client: Vec<String>,
    /// Jar trong `mods/` mà `.mrpack` ghi `env.server = "unsupported"`: không tải mặc định, nhưng
    /// MineHost tải lại khi nó là dependency bắt buộc của mod chạy trên server (spec P3, mục 3.5).
    pub server_unsupported: Vec<PackFile>,
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
    let mut archive = open_pack_archive(archive_path, limits)?;
    let (index, runtime) = read_index(&mut archive, archive_path, limits)?;
    let IndexPlan {
        files,
        skipped_client,
        server_unsupported,
        planned_bytes: mut index_bytes,
        mut index_count,
    } = plan_index_files(index.files, hosts, limits)?;
    // Quét trước để bắt zip bomb / tên file không hợp lệ trước khi tải loader hay file.
    let _ = scan_prefixed_layers(
        &mut archive,
        archive_path,
        Some(INDEX_NAME),
        &["overrides/", "server-overrides/"],
        limits,
        &mut index_count,
        &mut index_bytes,
    )?;

    Ok(PackPlan {
        name: index.name,
        version_id: index.version_id,
        loader: runtime.loader,
        minecraft: runtime.minecraft,
        loader_version: runtime.loader_version,
        files,
        skipped_client,
        server_unsupported,
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
    let mut archive = open_pack_archive(archive_path, limits)?;
    let (index, _) = read_index(&mut archive, archive_path, limits)?;
    let mut index_count = index.files.len();
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
    let mut scan_bytes = index_bytes;
    let layers = scan_prefixed_layers(
        &mut archive,
        archive_path,
        Some(INDEX_NAME),
        &["overrides/", "server-overrides/"],
        limits,
        &mut index_count,
        &mut scan_bytes,
    )?;
    // Luôn giải nén `overrides/` trước rồi mới tới `server-overrides/` để ghi đè đúng thứ tự.
    extract_entries(
        &mut archive,
        archive_path,
        dest_dir,
        layers.into_iter().flatten(),
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

fn read_index(
    archive: &mut zip::ZipArchive<std::io::BufReader<std::fs::File>>,
    archive_path: &Path,
    limits: PackLimits,
) -> Result<(MrpackIndex, MrpackRuntime), ProviderError> {
    let text = read_json_entry(archive, archive_path, INDEX_NAME, limits.max_index_bytes)?;
    let index =
        MrpackIndex::from_json(&text).map_err(|e| ProviderError::modpack_invalid(e.to_string()))?;
    let runtime = index
        .resolve_runtime()
        .map_err(|e| ProviderError::modpack_invalid(e.to_string()))?;
    Ok((index, runtime))
}

struct IndexPlan {
    files: Vec<PackFile>,
    skipped_client: Vec<String>,
    server_unsupported: Vec<PackFile>,
    planned_bytes: u64,
    index_count: usize,
}

fn plan_index_files(
    entries: Vec<mh_core::MrpackFile>,
    hosts: HostAllowlist,
    limits: PackLimits,
) -> Result<IndexPlan, ProviderError> {
    let index_count = entries.len();
    if index_count > limits.max_files {
        return Err(ProviderError::modpack_invalid(
            "số lượng file trong modpack vượt giới hạn",
        ));
    }
    let mut plan = IndexPlan {
        files: Vec::new(),
        skipped_client: Vec::new(),
        server_unsupported: Vec::new(),
        planned_bytes: 0,
        index_count,
    };
    // File dự phòng có ngân sách riêng, cùng mức trần với pack; vượt thì bỏ bớt dự phòng thay vì
    // từ chối pack, vì chúng chỉ được tải khi thật sự cần.
    let mut fallback_bytes = 0u64;
    for item in dedupe_entries(entries)? {
        validate_archive_path(&item.path)?;
        if item.is_client_only() {
            let within_budget = fallback_bytes
                .checked_add(item.file_size)
                .filter(|total| *total <= limits.max_total_bytes);
            if let Some(total) = within_budget.filter(|_| is_mod_jar(&item.path)) {
                // Chỉ là dự phòng: file hỏng (thiếu URL, sai host) thì bỏ, không làm hỏng cả pack.
                match pack_file(&item, hosts) {
                    Ok(file) => {
                        fallback_bytes = total;
                        plan.server_unsupported.push(file);
                    }
                    Err(err) => tracing::debug!(path = item.path, %err, "bỏ file dự phòng hỏng"),
                }
            }
            plan.skipped_client.push(item.path);
            continue;
        }
        plan.planned_bytes = add_within_limit(
            plan.planned_bytes,
            item.file_size,
            limits.max_total_bytes,
            "tổng kích thước modpack vượt giới hạn",
        )?;
        plan.files.push(pack_file(&item, hosts)?);
    }
    Ok(plan)
}

/// Có pack ghi cùng một file nhiều lần (Pokemon Elysium 1.2.1 lặp 4 lần); bản trùng y hệt chỉ cài
/// một lần. Cùng đường dẫn mà khác hash thì không biết chọn bản nào, nên từ chối pack.
fn dedupe_entries(
    entries: Vec<mh_core::MrpackFile>,
) -> Result<Vec<mh_core::MrpackFile>, ProviderError> {
    let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut out = Vec::with_capacity(entries.len());
    for item in entries {
        match seen.get(&item.path) {
            Some(hash) if hash.eq_ignore_ascii_case(&item.hashes.sha512) => continue,
            Some(_) => {
                return Err(ProviderError::modpack_invalid(format!(
                    "file `{}` xuất hiện nhiều lần với nội dung khác nhau",
                    item.path
                )));
            }
            None => {
                seen.insert(item.path.clone(), item.hashes.sha512.clone());
                out.push(item);
            }
        }
    }
    Ok(out)
}

fn is_mod_jar(path: &str) -> bool {
    path.starts_with("mods/")
        && !path["mods/".len()..].contains('/')
        && std::path::Path::new(path)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("jar"))
}

fn pack_file(item: &mh_core::MrpackFile, hosts: HostAllowlist) -> Result<PackFile, ProviderError> {
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
    Ok(PackFile {
        path: item.path.clone(),
        urls: item.downloads.clone(),
        checksum,
        sha1: sha1.hex().to_owned(),
        size: item.file_size,
        hosts,
        cf_ids: None,
    })
}
