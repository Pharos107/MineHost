//! Nén một thư mục thành `tar.zst` mức 3.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use tokio_util::sync::CancellationToken;
use zstd::stream::write::Encoder;

use crate::ArchiveInfo;
use crate::error::BackupError;
use crate::io_util::{self, HashWriter};

/// Mức nén zstd. Mức 3 là mặc định của thư viện, đủ nhanh cho world lớn.
const ZSTD_LEVEL: i32 = 3;

/// Thư mục Minecraft không cần đưa vào backup.
///
/// Khớp theo thành phần đường dẫn tính từ gốc `src_dir`: `logs/latest.log` khớp
/// `logs/`, còn `logs-extra/` và `plugins/logs/` thì không (không so chuỗi con).
pub const DEFAULT_EXCLUDE: &[&str] = &["logs/", "crash-reports/", "cache/"];

struct Item {
    rel: PathBuf,
    mode: u32,
    len: u64,
    is_dir: bool,
}

/// Nén `src_dir` vào `dest`.
///
/// Ghi `<dest>.part`, tính sha256 trên đúng byte ghi ra file, `fsync`, rồi `rename`.
/// Hủy hoặc lỗi thì xóa `.part`. Symlink bị bỏ qua, không đọc chỗ nó trỏ tới.
/// `progress(đã_đọc, tổng)` đếm byte nội dung file (chưa nén); tổng được tính bằng
/// một lần duyệt trước. Hàm đồng bộ — gọi trong `spawn_blocking`.
pub fn create(
    src_dir: &Path,
    dest: &Path,
    exclude: &[&str],
    mut progress: impl FnMut(u64, u64),
    cancel: &CancellationToken,
) -> Result<ArchiveInfo, BackupError> {
    if cancel.is_cancelled() {
        return Err(BackupError::Cancelled);
    }
    let rules = parse_rules(exclude);
    let (items, total) = collect(src_dir, &rules, cancel)?;
    let part = io_util::part_path(dest);
    let result = write_part(src_dir, dest, &part, &items, total, &mut progress, cancel);
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

fn collect(
    root: &Path,
    rules: &[Vec<String>],
    cancel: &CancellationToken,
) -> Result<(Vec<Item>, u64), BackupError> {
    let mut items = Vec::new();
    let mut total = 0u64;
    walk(root, Path::new(""), rules, cancel, &mut items, &mut total)?;
    Ok((items, total))
}

fn walk(
    dir: &Path,
    parent: &Path,
    rules: &[Vec<String>],
    cancel: &CancellationToken,
    items: &mut Vec<Item>,
    total: &mut u64,
) -> Result<(), BackupError> {
    if cancel.is_cancelled() {
        return Err(BackupError::Cancelled);
    }
    for entry in read_sorted(dir)? {
        let rel = join_rel(parent, &entry.file_name());
        if is_excluded(&rel, rules) {
            continue;
        }
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| BackupError::io(&path, e))?;
        let ft = meta.file_type();
        if ft.is_symlink() {
            // Symlink có thể trỏ ra ngoài thư mục server.
            tracing::debug!(path = %path.display(), "bỏ qua symlink khi nén backup");
            continue;
        }
        if ft.is_dir() {
            items.push(Item::dir(&rel, mode_of(&meta, true)));
            walk(&path, &rel, rules, cancel, items, total)?;
        } else if ft.is_file() {
            *total += meta.len();
            items.push(Item::file(&rel, mode_of(&meta, false), meta.len()));
        } else {
            tracing::debug!(path = %path.display(), "bỏ qua file đặc biệt khi nén backup");
        }
    }
    Ok(())
}

fn read_sorted(dir: &Path) -> Result<Vec<fs::DirEntry>, BackupError> {
    let mut entries = fs::read_dir(dir)
        .map_err(|e| BackupError::io(dir, e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| BackupError::io(dir, e))?;
    // Tên sắp xếp cố định để hai lần nén cùng một cây ra cùng byte.
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}

fn write_part(
    src_dir: &Path,
    dest: &Path,
    part: &Path,
    items: &[Item],
    total: u64,
    progress: &mut dyn FnMut(u64, u64),
    cancel: &CancellationToken,
) -> Result<ArchiveInfo, BackupError> {
    if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| BackupError::io(parent, e))?;
    }
    let file = File::create(part).map_err(|e| BackupError::io(part, e))?;
    let hashed = HashWriter::new(file);
    let mut encoder = Encoder::new(hashed, ZSTD_LEVEL).map_err(|e| BackupError::io(part, e))?;
    write_tar(&mut encoder, src_dir, items, total, progress, cancel)?;
    let hashed = encoder.finish().map_err(|e| map_io(part, e))?;
    finish_part(hashed, part, dest, cancel)
}

fn write_tar<W: Write>(
    encoder: &mut W,
    src_dir: &Path,
    items: &[Item],
    total: u64,
    progress: &mut dyn FnMut(u64, u64),
    cancel: &CancellationToken,
) -> Result<(), BackupError> {
    let mut builder = tar::Builder::new(encoder);
    let mut done = 0u64;
    for item in items {
        if cancel.is_cancelled() {
            return Err(BackupError::Cancelled);
        }
        append_item(
            &mut builder,
            src_dir,
            item,
            &mut done,
            total,
            progress,
            cancel,
        )?;
    }
    builder.finish().map_err(|e| map_io(src_dir, e))?;
    Ok(())
}

fn finish_part(
    mut hashed: HashWriter<File>,
    part: &Path,
    dest: &Path,
    cancel: &CancellationToken,
) -> Result<ArchiveInfo, BackupError> {
    hashed.flush().map_err(|e| BackupError::io(part, e))?;
    let (file, size, sha256) = hashed.into_parts();
    file.sync_all().map_err(|e| BackupError::io(part, e))?;
    drop(file);
    if cancel.is_cancelled() {
        return Err(BackupError::Cancelled);
    }
    fs::rename(part, dest).map_err(|e| BackupError::io(dest, e))?;
    Ok(ArchiveInfo { size, sha256 })
}

fn append_item<W: Write>(
    builder: &mut tar::Builder<W>,
    src_dir: &Path,
    item: &Item,
    done: &mut u64,
    total: u64,
    progress: &mut dyn FnMut(u64, u64),
    cancel: &CancellationToken,
) -> Result<(), BackupError> {
    let abs = src_dir.join(&item.rel);
    if fs::symlink_metadata(&abs)
        .map_err(|e| BackupError::io(&abs, e))?
        .file_type()
        .is_symlink()
    {
        tracing::debug!(path = %abs.display(), "bỏ qua symlink khi nén backup");
        return Ok(());
    }
    let archive_path = to_archive_path(&item.rel)?;
    if item.is_dir {
        let mut header = gnu_header(tar::EntryType::Directory, 0, item.mode);
        return builder
            .append_data(&mut header, format!("{archive_path}/"), io::empty())
            .map_err(|e| map_io(&abs, e));
    }
    let file = File::open(&abs).map_err(|e| BackupError::io(&abs, e))?;
    let mut header = gnu_header(tar::EntryType::Regular, item.len, item.mode);
    let mut reader = CancelRead {
        inner: file,
        cancel,
        done,
        total,
        left: item.len,
        progress,
    };
    builder
        .append_data(&mut header, archive_path, &mut reader)
        .map_err(|e| map_io(&abs, e))
}

/// `mtime` bằng 0 để hash không đổi chỉ vì file bị chạm thời điểm sửa.
fn gnu_header(kind: tar::EntryType, size: u64, mode: u32) -> tar::Header {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(kind);
    header.set_size(size);
    header.set_mode(mode);
    header.set_mtime(0);
    header.set_uid(0);
    header.set_gid(0);
    header.set_cksum();
    header
}

struct CancelRead<'a, R> {
    inner: R,
    cancel: &'a CancellationToken,
    done: &'a mut u64,
    total: u64,
    left: u64,
    progress: &'a mut dyn FnMut(u64, u64),
}

impl<R: Read> Read for CancelRead<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(io_util::cancel_err());
        }
        if self.left == 0 {
            return Ok(0);
        }
        let max = usize::try_from(self.left)
            .unwrap_or(usize::MAX)
            .min(buf.len());
        let n = self.inner.read(&mut buf[..max])?;
        if n == 0 {
            // File ngắn hơn lúc duyệt: header đã ghi kích thước cũ, dừng ở đây
            // sẽ làm archive lệch. Trả lỗi để xóa `.part`.
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "file ngắn hơn lúc duyệt thư mục",
            ));
        }
        self.left -= n as u64;
        *self.done += n as u64;
        (self.progress)(*self.done, self.total);
        if self.cancel.is_cancelled() {
            return Err(io_util::cancel_err());
        }
        Ok(n)
    }
}

fn parse_rules(exclude: &[&str]) -> Vec<Vec<String>> {
    exclude
        .iter()
        .map(|rule| {
            rule.split(['/', '\\'])
                .filter(|part| !part.is_empty() && *part != ".")
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|parts| !parts.is_empty())
        .collect()
}

fn is_excluded(rel: &Path, rules: &[Vec<String>]) -> bool {
    let comps = path_parts(rel);
    rules.iter().any(|rule| {
        comps.len() >= rule.len() && comps.iter().zip(rule).all(|(got, want)| got == want)
    })
}

fn path_parts(rel: &Path) -> Vec<String> {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(name) => name.to_str().map(str::to_owned),
            _ => None,
        })
        .collect()
}

fn to_archive_path(rel: &Path) -> Result<String, BackupError> {
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(name) => {
                let name = name.to_str().ok_or_else(|| {
                    BackupError::io(
                        rel,
                        io::Error::new(io::ErrorKind::InvalidData, "tên file không phải Unicode"),
                    )
                })?;
                parts.push(name);
            }
            _ => return Err(BackupError::PathOutsideRoot(rel.display().to_string())),
        }
    }
    Ok(parts.join("/"))
}

fn join_rel(parent: &Path, name: &std::ffi::OsStr) -> PathBuf {
    if parent.as_os_str().is_empty() {
        PathBuf::from(name)
    } else {
        parent.join(name)
    }
}

fn mode_of(meta: &fs::Metadata, is_dir: bool) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode() & 0o777;
        if mode == 0 {
            return if is_dir { 0o755 } else { 0o644 };
        }
        mode
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        if is_dir { 0o755 } else { 0o644 }
    }
}

fn map_io(path: &Path, err: io::Error) -> BackupError {
    if io_util::is_cancel(&err) {
        BackupError::Cancelled
    } else {
        BackupError::io(path, err)
    }
}

impl Item {
    fn dir(rel: &Path, mode: u32) -> Self {
        Self {
            rel: rel.to_path_buf(),
            mode,
            len: 0,
            is_dir: true,
        }
    }

    fn file(rel: &Path, mode: u32, len: u64) -> Self {
        Self {
            rel: rel.to_path_buf(),
            mode,
            len,
            is_dir: false,
        }
    }
}
