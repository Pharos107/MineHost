//! Giải nén `tar.zst`. Hash được kiểm tra trước khi tạo file trong thư mục đích.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

use mh_platform::{PlatformError, SafeRoot};
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;
use zstd::stream::read::Decoder;

use crate::error::BackupError;
use crate::io_util;

const CHUNK: usize = 64 * 1024;

/// Kiểm tra sha256 cả file rồi giải nén vào `dest_dir`.
///
/// `dest_dir` phải chưa tồn tại hoặc đang rỗng. Sai hash thì không tạo gì
/// (`ErrorCode::BackupCorrupt`). Entry có `..` hoặc đường dẫn tuyệt đối làm hàm
/// trả `PathOutsideRoot` sau khi duyệt hết archive; symlink, hardlink, FIFO và
/// file thiết bị bị bỏ qua. `progress` lúc kiểm tra hash là `(đã đọc, kích thước
/// file nén)`, lúc ghi file là `(byte đã ghi, 0)` vì chưa biết trước kích thước
/// sau giải nén. Hàm đồng bộ — gọi trong `spawn_blocking`.
pub fn restore(
    archive: &Path,
    expected_sha256: &str,
    dest_dir: &Path,
    mut progress: impl FnMut(u64, u64),
    cancel: &CancellationToken,
) -> Result<(), BackupError> {
    if cancel.is_cancelled() {
        return Err(BackupError::Cancelled);
    }
    let actual = hash_file(archive, &mut progress, cancel)?;
    if !hex_eq(expected_sha256, &actual) {
        return Err(BackupError::Corrupt {
            expected: expected_sha256.to_owned(),
            actual,
        });
    }
    let file = File::open(archive).map_err(|e| BackupError::io(archive, e))?;
    let decoder = Decoder::new(file).map_err(|e| BackupError::io(archive, e))?;
    unpack_tar(decoder, dest_dir, progress, cancel)
}

/// Giải nén một luồng **tar chưa nén** vào `dest_dir`, không kiểm tra sha256.
///
/// `restore` gọi hàm này sau khi hash khớp. Fuzz đưa byte ngẫu nhiên vào đây.
/// Cùng quy tắc đường dẫn với `restore`.
pub fn unpack_tar(
    reader: impl Read,
    dest_dir: &Path,
    mut progress: impl FnMut(u64, u64),
    cancel: &CancellationToken,
) -> Result<(), BackupError> {
    if cancel.is_cancelled() {
        return Err(BackupError::Cancelled);
    }
    let root = prepare_dest(dest_dir)?;
    let mut archive = tar::Archive::new(reader);
    let mut written = 0u64;
    let mut ctx = UnpackCtx {
        written: &mut written,
        progress: &mut progress,
        cancel,
    };
    let mut rejected: Option<BackupError> = None;
    // Không dùng `Entry::unpack`: hàm đó tạo symlink và không đi qua `SafeRoot`.
    for entry in archive.entries().map_err(archive_err)? {
        if ctx.cancel.is_cancelled() {
            return Err(BackupError::Cancelled);
        }
        let mut entry = entry.map_err(archive_err)?;
        match handle_entry(&root, &mut entry, &mut ctx) {
            Ok(()) => {}
            Err(err @ BackupError::PathOutsideRoot(_)) if rejected.is_none() => {
                rejected = Some(err);
            }
            Err(BackupError::PathOutsideRoot(_)) => {}
            Err(err) => return Err(err),
        }
    }
    match rejected {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

fn hash_file(
    path: &Path,
    progress: &mut dyn FnMut(u64, u64),
    cancel: &CancellationToken,
) -> Result<String, BackupError> {
    let mut file = File::open(path).map_err(|e| BackupError::io(path, e))?;
    let len = file.metadata().map_err(|e| BackupError::io(path, e))?.len();
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    let mut buf = [0u8; CHUNK];
    loop {
        if cancel.is_cancelled() {
            return Err(BackupError::Cancelled);
        }
        let n = file.read(&mut buf).map_err(|e| BackupError::io(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        done += n as u64;
        progress(done, len);
        if cancel.is_cancelled() {
            return Err(BackupError::Cancelled);
        }
    }
    Ok(io_util::hex(&hasher.finalize()))
}

fn hex_eq(expected: &str, actual: &str) -> bool {
    expected.len() == actual.len() && expected.eq_ignore_ascii_case(actual)
}

fn prepare_dest(dest: &Path) -> Result<SafeRoot, BackupError> {
    match fs::create_dir(dest) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => ensure_empty(dest)?,
        Err(err) => return Err(BackupError::io(dest, err)),
    }
    Ok(SafeRoot::new(dest)?)
}

fn ensure_empty(dest: &Path) -> Result<(), BackupError> {
    let mut entries = fs::read_dir(dest).map_err(|e| BackupError::io(dest, e))?;
    match entries.next() {
        None => Ok(()),
        Some(Ok(_)) => Err(BackupError::DestNotEmpty(dest.to_path_buf())),
        Some(Err(err)) => Err(BackupError::io(dest, err)),
    }
}

enum Plan {
    Write {
        rel: String,
        dir: bool,
        mode: Option<u32>,
    },
    Skip,
    Reject(BackupError),
}

struct UnpackCtx<'a> {
    written: &'a mut u64,
    progress: &'a mut dyn FnMut(u64, u64),
    cancel: &'a CancellationToken,
}

fn handle_entry<R: Read>(
    root: &SafeRoot,
    entry: &mut tar::Entry<'_, R>,
    ctx: &mut UnpackCtx<'_>,
) -> Result<(), BackupError> {
    match plan_entry(entry)? {
        Plan::Skip => discard(entry, ctx.cancel),
        Plan::Reject(err) => {
            // Đọc bỏ phần thân để entry sau không bị lệch khung tar.
            discard(entry, ctx.cancel)?;
            Err(err)
        }
        Plan::Write { rel, dir, mode } => write_planned(root, entry, &rel, dir, mode, ctx),
    }
}

fn plan_entry<R: Read>(entry: &tar::Entry<'_, R>) -> Result<Plan, BackupError> {
    let path = match entry.path() {
        Ok(path) => path,
        Err(_) => {
            return Ok(Plan::Reject(BackupError::PathOutsideRoot(
                "đường dẫn không đọc được".into(),
            )));
        }
    };
    let rel = match normalize_entry(path.as_ref()) {
        Ok(rel) => rel,
        Err(err) => return Ok(Plan::Reject(err)),
    };
    if rel.is_empty() {
        return Ok(Plan::Skip);
    }
    let kind = entry.header().entry_type();
    let mode = entry.header().mode().ok();
    if kind == tar::EntryType::Directory {
        return Ok(Plan::Write {
            rel,
            dir: true,
            mode,
        });
    }
    if is_regular(kind) {
        return Ok(Plan::Write {
            rel,
            dir: false,
            mode,
        });
    }
    tracing::debug!(name = %rel, "bỏ qua symlink, hardlink hoặc file đặc biệt khi giải nén");
    Ok(Plan::Skip)
}

fn write_planned<R: Read>(
    root: &SafeRoot,
    entry: &mut tar::Entry<'_, R>,
    rel: &str,
    dir: bool,
    mode: Option<u32>,
    ctx: &mut UnpackCtx<'_>,
) -> Result<(), BackupError> {
    let target = match join_safe(root, rel) {
        Ok(path) => path,
        Err(err) => {
            // Vòng lặp còn đọc entry sau khi `PathOutsideRoot`, nên phải xả hết thân.
            discard(entry, ctx.cancel)?;
            return Err(err);
        }
    };
    if dir {
        fs::create_dir_all(&target).map_err(|e| BackupError::io(&target, e))?;
        set_mode(&target, mode)?;
        return discard(entry, ctx.cancel);
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| BackupError::io(parent, e))?;
    }
    let mut out = File::create(&target).map_err(|e| BackupError::io(&target, e))?;
    // `cancel` là `&CancellationToken` (Copy). Mượn `ctx` cùng lúc với closure sẽ không dịch được.
    let cancel = ctx.cancel;
    transfer(entry, &mut out, &target, cancel, &mut |n| {
        *ctx.written += n as u64;
        (ctx.progress)(*ctx.written, 0);
    })?;
    drop(out);
    set_mode(&target, mode)
}

/// `SafeRoot` coi `/abs` là gốc thư mục đích và nuốt `a/../b` thành `b`.
/// Trong tar, cả hai đều là đường dẫn không được chuẩn hóa hộ.
fn normalize_entry(path: &Path) -> Result<String, BackupError> {
    let raw = path
        .to_str()
        .ok_or_else(|| BackupError::PathOutsideRoot(path.to_string_lossy().into_owned()))?;
    if raw.starts_with('/') || raw.starts_with('\\') || raw.contains(':') {
        return Err(BackupError::PathOutsideRoot(raw.to_owned()));
    }
    let mut parts = Vec::new();
    for part in raw.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return Err(BackupError::PathOutsideRoot(raw.to_owned())),
            name => parts.push(name),
        }
    }
    Ok(parts.join("/"))
}

fn join_safe(root: &SafeRoot, rel: &str) -> Result<std::path::PathBuf, BackupError> {
    match root.join(rel) {
        Ok(path) => Ok(path.into_path_buf()),
        Err(PlatformError::PathOutsideRoot(path)) => Err(BackupError::PathOutsideRoot(path)),
        Err(other) => Err(other.into()),
    }
}

fn is_regular(kind: tar::EntryType) -> bool {
    matches!(kind, tar::EntryType::Regular | tar::EntryType::Continuous)
}

fn discard<R: Read>(
    entry: &mut tar::Entry<'_, R>,
    cancel: &CancellationToken,
) -> Result<(), BackupError> {
    let mut sink = io::sink();
    transfer(entry, &mut sink, Path::new("archive"), cancel, &mut |_| {})
}

fn transfer<R: Read, W: Write>(
    src: &mut tar::Entry<'_, R>,
    dst: &mut W,
    dst_path: &Path,
    cancel: &CancellationToken,
    on_bytes: &mut dyn FnMut(usize),
) -> Result<(), BackupError> {
    let mut buf = [0u8; CHUNK];
    loop {
        if cancel.is_cancelled() {
            return Err(BackupError::Cancelled);
        }
        let n = src.read(&mut buf).map_err(archive_err)?;
        if n == 0 {
            return Ok(());
        }
        dst.write_all(&buf[..n])
            .map_err(|e| BackupError::io(dst_path, e))?;
        on_bytes(n);
        if cancel.is_cancelled() {
            return Err(BackupError::Cancelled);
        }
    }
}

fn archive_err(err: io::Error) -> BackupError {
    if io_util::is_cancel(&err) {
        BackupError::Cancelled
    } else {
        BackupError::Archive(err.to_string())
    }
}

/// Giữ bit thực thi của script trên Linux, giống lúc giải nén Java.
#[cfg(unix)]
fn set_mode(target: &Path, mode: Option<u32>) -> Result<(), BackupError> {
    use std::os::unix::fs::PermissionsExt;
    let Some(mode) = mode else {
        return Ok(());
    };
    fs::set_permissions(target, fs::Permissions::from_mode(mode & 0o777))
        .map_err(|e| BackupError::io(target, e))
}

#[cfg(not(unix))]
fn set_mode(_target: &Path, _mode: Option<u32>) -> Result<(), BackupError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::normalize_entry;
    use crate::BackupError;

    #[test]
    fn rejects_parent_and_absolute_entries() {
        for raw in [
            "../evil",
            "/abs",
            r"\abs",
            "foo/../../x",
            "C:/Windows/a",
            "a/../b",
        ] {
            assert!(
                matches!(
                    normalize_entry(Path::new(raw)),
                    Err(BackupError::PathOutsideRoot(_))
                ),
                "{raw}"
            );
        }
    }

    #[test]
    fn keeps_normal_relative_entries() {
        let rel = normalize_entry(Path::new("./thế giới/level.dat")).unwrap();
        assert_eq!(rel, "thế giới/level.dat");
        assert_eq!(
            normalize_entry(Path::new("logs-extra/a")).unwrap(),
            "logs-extra/a"
        );
    }
}
