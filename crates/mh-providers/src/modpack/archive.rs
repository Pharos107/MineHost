//! Hỗ trợ đọc/giải nén an toàn các entry trong archive `.mrpack`.

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

pub(super) fn extract_overrides(
    archive: &mut ZipArchive<BufReader<File>>,
    archive_path: &Path,
    root: &SafeRoot,
    overrides: Vec<OverrideEntry>,
    server_overrides: Vec<OverrideEntry>,
    limits: PackLimits,
    mut actual_bytes: u64,
) -> Result<(), ProviderError> {
    let mut buf = vec![0u8; COPY_BUF_BYTES];
    // Luôn giải nén `overrides/` trước rồi mới tới `server-overrides/` để ghi đè đúng thứ tự.
    for item in overrides.into_iter().chain(server_overrides) {
        let target = safe_join(root, &item.rel)?;
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
