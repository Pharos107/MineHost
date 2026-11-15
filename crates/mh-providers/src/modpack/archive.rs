//! Hỗ trợ đọc/giải nén an toàn các entry trong archive modpack (`.mrpack` và `.zip`).

use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::path::Path;

use mh_core::validate_mrpack_path;
use mh_platform::safe_path::is_valid_name;
use mh_platform::{PlatformError, SafePath, SafeRoot};
use zip::ZipArchive;
use zip::read::ZipFile;
use zip::result::ZipError;

use super::{COPY_BUF_BYTES, OverrideEntry, PackLimits};
use crate::ProviderError;

pub(super) struct RawZipEntry {
    pub index: usize,
    pub name: String,
    pub is_dir: bool,
    pub skip_special: bool,
    pub size: u64,
}

pub(super) fn open_pack_archive(
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

pub(super) fn read_json_entry(
    archive: &mut ZipArchive<BufReader<File>>,
    archive_path: &Path,
    entry_name: &str,
    max_bytes: u64,
) -> Result<String, ProviderError> {
    let entry = match archive.by_name(entry_name) {
        Ok(entry) => entry,
        Err(ZipError::FileNotFound) => {
            return Err(ProviderError::modpack_invalid(format!(
                "thiếu {entry_name}"
            )));
        }
        Err(e) => return Err(zip_err(archive_path, e)),
    };
    if entry.is_dir() || is_unix_dir(&entry) || is_symlink_or_special(&entry) {
        return Err(ProviderError::modpack_invalid(format!(
            "{entry_name} không phải file thường"
        )));
    }
    if entry.size() > max_bytes {
        return Err(ProviderError::modpack_invalid(format!(
            "{entry_name} vượt quá giới hạn kích thước"
        )));
    }
    let mut buf = Vec::new();
    entry
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(|e| map_read_io(archive_path, e))?;
    if buf.len() as u64 > max_bytes {
        return Err(ProviderError::modpack_invalid(format!(
            "{entry_name} vượt quá giới hạn kích thước"
        )));
    }
    String::from_utf8(buf)
        .map_err(|_| ProviderError::modpack_invalid(format!("{entry_name} không phải UTF-8")))
}

pub(super) fn read_entry_meta(
    archive: &mut ZipArchive<BufReader<File>>,
    index: usize,
    archive_path: &Path,
) -> Result<RawZipEntry, ProviderError> {
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
    let raw_check = if is_dir {
        name.strip_suffix('/').unwrap_or(&name)
    } else {
        &name
    };
    validate_archive_path(raw_check)?;
    Ok(RawZipEntry {
        index,
        name,
        is_dir,
        skip_special,
        size,
    })
}

pub(super) fn scan_prefixed_layers(
    archive: &mut ZipArchive<BufReader<File>>,
    archive_path: &Path,
    ignore_name: Option<&str>,
    prefixes: &[&str],
    limits: PackLimits,
    total_files: &mut usize,
    declared_bytes: &mut u64,
) -> Result<Vec<Vec<OverrideEntry>>, ProviderError> {
    let mut layers: Vec<Vec<OverrideEntry>> = (0..prefixes.len()).map(|_| Vec::new()).collect();
    for index in 0..archive.len() {
        let meta = read_entry_meta(archive, index, archive_path)?;
        if ignore_name == Some(meta.name.as_str()) {
            continue;
        }
        if meta.skip_special {
            tracing::debug!(name = %meta.name, "bỏ qua symlink hoặc file đặc biệt trong modpack");
            continue;
        }
        let Some((layer_idx, rel)) = prefixes
            .iter()
            .enumerate()
            .find_map(|(i, p)| meta.name.strip_prefix(p).map(|rel| (i, rel)))
        else {
            continue;
        };
        let rel = if meta.is_dir {
            rel.strip_suffix('/').unwrap_or(rel)
        } else {
            rel
        };
        if rel.is_empty() {
            continue;
        }
        count_within_limits(meta.is_dir, meta.size, limits, total_files, declared_bytes)?;
        layers[layer_idx].push(OverrideEntry {
            index,
            rel: rel.to_owned(),
            is_dir: meta.is_dir,
        });
    }
    Ok(layers)
}

pub(super) fn count_within_limits(
    is_dir: bool,
    size: u64,
    limits: PackLimits,
    total_files: &mut usize,
    declared_bytes: &mut u64,
) -> Result<(), ProviderError> {
    if is_dir {
        return Ok(());
    }
    *total_files = total_files
        .checked_add(1)
        .filter(|&n| n <= limits.max_files)
        .ok_or_else(|| {
            ProviderError::modpack_invalid("số lượng file trong modpack vượt giới hạn")
        })?;
    *declared_bytes = add_within_limit(
        *declared_bytes,
        size,
        limits.max_total_bytes,
        "tổng kích thước modpack vượt giới hạn",
    )?;
    Ok(())
}

pub(super) fn extract_entries(
    archive: &mut ZipArchive<BufReader<File>>,
    archive_path: &Path,
    dest_dir: &Path,
    entries: impl IntoIterator<Item = OverrideEntry>,
    limits: PackLimits,
    mut actual_bytes: u64,
) -> Result<(), ProviderError> {
    fs::create_dir_all(dest_dir).map_err(|e| ProviderError::io(dest_dir, e))?;
    let root = SafeRoot::new(dest_dir)?;
    let mut buf = vec![0u8; COPY_BUF_BYTES];

    for item in entries {
        // Chỉ người dùng được đồng ý EULA trong MineHost; không nhận `eula=true` đi kèm pack.
        if is_root_eula(&item.rel) {
            tracing::debug!(path = %item.rel, "bỏ qua eula.txt trong modpack");
            continue;
        }
        let target = safe_join(&root, &item.rel)?;
        if item.is_dir {
            fs::create_dir_all(target.as_path())
                .map_err(|e| ProviderError::io(target.as_path(), e))?;
            continue;
        }
        if let Some(parent) = target.as_path().parent() {
            fs::create_dir_all(parent).map_err(|e| ProviderError::io(parent, e))?;
        }
        let mut entry = archive
            .by_index(item.index)
            .map_err(|e| zip_err(archive_path, e))?;
        let mut out =
            File::create(target.as_path()).map_err(|e| ProviderError::io(target.as_path(), e))?;
        copy_bounded(
            &mut entry,
            &mut out,
            archive_path,
            target.as_path(),
            &mut buf,
            &mut actual_bytes,
            limits.max_total_bytes,
        )?;
    }
    Ok(())
}

fn is_root_eula(rel: &str) -> bool {
    rel.eq_ignore_ascii_case("eula.txt")
}

fn copy_bounded(
    entry: &mut ZipFile<'_, BufReader<File>>,
    out: &mut File,
    archive_path: &Path,
    target: &Path,
    buf: &mut [u8],
    actual_bytes: &mut u64,
    max_total: u64,
) -> Result<(), ProviderError> {
    let remaining = max_total.saturating_sub(*actual_bytes);
    let mut limited = entry.take(remaining.saturating_add(1));
    loop {
        let n = limited
            .read(buf)
            .map_err(|e| map_read_io(archive_path, e))?;
        if n == 0 {
            return Ok(());
        }
        *actual_bytes = add_within_limit(
            *actual_bytes,
            n as u64,
            max_total,
            "kích thước giải nén thực tế vượt giới hạn",
        )?;
        out.write_all(&buf[..n])
            .map_err(|e| ProviderError::io(target, e))?;
    }
}

pub(super) fn add_within_limit(
    total: u64,
    delta: u64,
    max: u64,
    msg: &str,
) -> Result<u64, ProviderError> {
    total
        .checked_add(delta)
        .filter(|&sum| sum <= max)
        .ok_or_else(|| ProviderError::modpack_invalid(msg))
}

pub(super) fn validate_archive_path(path: &str) -> Result<(), ProviderError> {
    validate_mrpack_path(path).map_err(|e| ProviderError::modpack_invalid(e.to_string()))?;
    if path.split('/').any(|seg| !is_valid_name(seg)) {
        return Err(ProviderError::modpack_invalid(format!(
            "đường dẫn `{path}` dùng tên không hợp lệ"
        )));
    }
    Ok(())
}

fn safe_join(root: &SafeRoot, rel: &str) -> Result<SafePath, ProviderError> {
    root.join(rel).map_err(|e| match e {
        PlatformError::PathOutsideRoot(_) | PlatformError::InvalidPath(_) => {
            ProviderError::modpack_invalid(e.to_string())
        }
        other => ProviderError::Platform(other),
    })
}

pub(super) fn is_unix_dir<R: Read + ?Sized>(entry: &ZipFile<'_, R>) -> bool {
    entry
        .unix_mode()
        .is_some_and(|mode| (mode & 0o170000) == 0o040000)
}

pub(super) fn is_symlink_or_special<R: Read + ?Sized>(entry: &ZipFile<'_, R>) -> bool {
    if entry.is_symlink() {
        return true;
    }
    entry.unix_mode().is_some_and(|mode| {
        let kind = mode & 0o170000;
        kind != 0 && kind != 0o100000 && kind != 0o040000
    })
}

pub(super) fn zip_err(path: &Path, err: ZipError) -> ProviderError {
    match err {
        ZipError::Io(e) => map_read_io(path, e),
        other => ProviderError::modpack_invalid(other.to_string()),
    }
}

pub(super) fn map_read_io(path: &Path, err: io::Error) -> ProviderError {
    if matches!(
        err.kind(),
        io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof
    ) {
        ProviderError::modpack_invalid(err.to_string())
    } else {
        ProviderError::io(path, err)
    }
}
