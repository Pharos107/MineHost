//! Zip của trang Tệp: tải nhiều mục thành một zip, nén, giải nén (spec trang Tệp, mục 3).
//! Đồng bộ, chạy trong `spawn_blocking`. Mọi đường dẫn đi qua `SafeRoot`.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

use mh_core::ErrorCode;
use mh_platform::SafeRoot;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::disk::{Node, ensure_parent_dir, join_user, joined_file, node_at};
use super::{AppError, child_path, exists, invalid, io_err, outside};

/// Zip có nhiều mục hơn thế bị từ chối khi giải nén.
pub const EXTRACT_MAX_ENTRIES: usize = 100_000;
/// Chừa lại trên đĩa sau khi giải nén, để server và log còn chỗ ghi.
const DISK_MARGIN: u64 = 1024 * 1024 * 1024;
const CHUNK: usize = 64 * 1024;

/// Báo số byte đã xử lý; trả lỗi khi Job bị hủy để dừng giữa chừng.
pub(crate) type Progress<'a> = &'a mut dyn FnMut(u64) -> Result<(), AppError>;

/// Danh sách mục sẽ nén, lập trước để báo lỗi (thiếu file, trùng tên) trước khi ghi gì.
#[derive(Debug)]
pub(crate) struct ZipPlan {
    entries: Vec<PlanEntry>,
    pub(crate) total: u64,
}

#[derive(Debug)]
struct PlanEntry {
    abs: PathBuf,
    /// Tên trong zip, ngăn bằng `/`; thư mục kết thúc bằng `/`.
    name: String,
    /// `None` là thư mục.
    size: Option<u64>,
}

pub(crate) fn plan_zip(root: &SafeRoot, paths: &[String]) -> Result<ZipPlan, AppError> {
    if paths.is_empty() {
        return Err(invalid("chưa chọn mục nào"));
    }
    let mut plan = ZipPlan {
        entries: Vec::new(),
        total: 0,
    };
    let mut names = HashSet::new();
    for user_path in paths {
        let path = joined_file(root, user_path)?;
        if node_at(path.as_path())? == Node::Missing {
            return Err(AppError::not_found(user_path));
        }
        let name = file_name(path.as_path())?;
        // Hai mục cùng tên ở hai thư mục khác nhau sẽ đè nhau trong zip.
        if !names.insert(name.clone()) {
            return Err(invalid("trùng tên trong các mục đã chọn"));
        }
        walk(path.as_path(), &name, &mut plan)?;
    }
    Ok(plan)
}

/// Thư mục duyệt đệ quy theo thứ tự tên; symlink bị bỏ qua để không nén thứ ngoài server.
fn walk(abs: &Path, name: &str, plan: &mut ZipPlan) -> Result<(), AppError> {
    let meta = fs::symlink_metadata(abs).map_err(|err| io_err(abs, err))?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    if !meta.is_dir() {
        plan.total += meta.len();
        plan.entries.push(PlanEntry {
            abs: abs.to_path_buf(),
            name: name.to_owned(),
            size: Some(meta.len()),
        });
        return Ok(());
    }
    plan.entries.push(PlanEntry {
        abs: abs.to_path_buf(),
        name: format!("{name}/"),
        size: None,
    });
    let mut children = Vec::new();
    for entry in fs::read_dir(abs).map_err(|err| io_err(abs, err))? {
        let entry = entry.map_err(|err| io_err(abs, err))?;
        if let Ok(child) = entry.file_name().into_string() {
            children.push(child);
        }
    }
    children.sort();
    for child in children {
        walk(&abs.join(&child), &format!("{name}/{child}"), plan)?;
    }
    Ok(())
}

/// Ghi mọi mục của `plan` vào `zip` (chưa `finish`).
pub(crate) fn write_zip<W: Write + Seek>(
    plan: &ZipPlan,
    zip: &mut ZipWriter<W>,
    progress: Progress<'_>,
) -> Result<(), AppError> {
    let mut done = 0u64;
    for entry in &plan.entries {
        let Some(size) = entry.size else {
            zip.add_directory(entry.name.trim_end_matches('/'), options(0))
                .map_err(zip_err)?;
            continue;
        };
        zip.start_file(entry.name.as_str(), options(size))
            .map_err(zip_err)?;
        let mut file = File::open(&entry.abs).map_err(|err| io_err(&entry.abs, err))?;
        let mut buf = vec![0u8; CHUNK];
        loop {
            let n = file.read(&mut buf).map_err(|err| io_err(&entry.abs, err))?;
            if n == 0 {
                break;
            }
            zip.write_all(&buf[..n]).map_err(write_err)?;
            done += n as u64;
            progress(done)?;
        }
    }
    Ok(())
}

fn options(size: u64) -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .large_file(size >= u64::from(u32::MAX))
        .unix_permissions(0o644)
}

/// Kiểm tra đích trước khi tạo Job, để `FILE_EXISTS` trả ngay cho người dùng.
pub(crate) fn compress_target(root: &SafeRoot, dest: &str) -> Result<PathBuf, AppError> {
    let path = joined_file(root, dest)?;
    if node_at(path.as_path())? != Node::Missing {
        return Err(exists(dest));
    }
    ensure_parent_dir(path.as_path())?;
    Ok(path.into_path_buf())
}

/// Nén vào `<đích>.part` rồi đổi tên; lỗi hoặc bị hủy thì xóa file dở.
pub(crate) fn compress_sync(
    plan: &ZipPlan,
    dest: &Path,
    progress: Progress<'_>,
) -> Result<(), AppError> {
    let mut part = dest.as_os_str().to_owned();
    part.push(".part");
    let part = PartFile(PathBuf::from(part));
    let file = File::create_new(&part.0).map_err(|err| io_err(&part.0, err))?;
    let mut zip = ZipWriter::new(BufWriter::new(file));
    write_zip(plan, &mut zip, progress)?;
    let writer = zip.finish().map_err(zip_err)?;
    let file = writer
        .into_inner()
        .map_err(|err| io_err(&part.0, err.into_error()))?;
    file.sync_all().map_err(|err| io_err(&part.0, err))?;
    drop(file);
    if dest.exists() {
        return Err(exists(&dest.display().to_string()));
    }
    fs::rename(&part.0, dest).map_err(|err| io_err(&part.0, err))?;
    part.keep();
    Ok(())
}

/// Xóa file `.part` khi chưa đổi tên xong (lỗi, hủy, panic).
struct PartFile(PathBuf);

impl PartFile {
    fn keep(mut self) {
        self.0 = PathBuf::new();
    }
}

impl Drop for PartFile {
    fn drop(&mut self) {
        if !self.0.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.0);
        }
    }
}

/// Mọi mục sẽ giải nén, đã kiểm tra an toàn và trùng tên.
#[derive(Debug)]
pub(crate) struct ExtractPlan {
    archive: ZipArchive<File>,
    dest: PathBuf,
    items: Vec<ExtractItem>,
    pub(crate) total: u64,
}

#[derive(Debug)]
struct ExtractItem {
    index: usize,
    target: PathBuf,
    /// `None` là thư mục.
    size: Option<u64>,
}

pub(crate) fn plan_extract(
    root: &SafeRoot,
    zip_path: &str,
    dest: &str,
    free_space: Option<u64>,
) -> Result<ExtractPlan, AppError> {
    let source = joined_file(root, zip_path)?;
    match node_at(source.as_path())? {
        Node::File => {}
        Node::Dir => return Err(invalid("không phải file zip")),
        Node::Missing => return Err(AppError::not_found(zip_path)),
    }
    let file = File::open(source.as_path()).map_err(|err| io_err(source.as_path(), err))?;
    let mut archive = ZipArchive::new(file).map_err(|_| invalid("không đọc được file zip"))?;
    if archive.len() > EXTRACT_MAX_ENTRIES {
        return Err(invalid("zip có quá nhiều mục"));
    }
    let dest_path = join_user(root, dest)?;
    if node_at(dest_path.as_path())? == Node::File {
        return Err(exists(dest));
    }
    let mut items = Vec::with_capacity(archive.len());
    let mut total = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index).map_err(zip_err)?;
        if entry.is_symlink() {
            continue;
        }
        let Some(rel) = entry.enclosed_name().and_then(|path| slash_path(&path)) else {
            let name = entry
                .name()
                .map(|name| name.into_owned())
                .unwrap_or_default();
            return Err(outside(&name));
        };
        let user_path = child_path(dest, &rel);
        let target = join_user(root, &user_path)?;
        let node = node_at(target.as_path())?;
        let size = (!entry.is_dir()).then(|| entry.size());
        // Thư mục đã có thì gộp vào; file nào đã có thì không ghi đè.
        if node == Node::File || (size.is_some() && node != Node::Missing) {
            return Err(exists(&user_path).with_params(serde_json::json!({ "path": user_path })));
        }
        total = total.saturating_add(size.unwrap_or(0));
        items.push(ExtractItem {
            index,
            target: target.into_path_buf(),
            size,
        });
    }
    if let Some(free) = free_space
        && total.saturating_add(DISK_MARGIN) > free
    {
        return Err(AppError::new(
            ErrorCode::NotEnoughDisk,
            format!("cần {total} byte, còn {free} byte"),
        ));
    }
    Ok(ExtractPlan {
        archive,
        dest: dest_path.into_path_buf(),
        items,
        total,
    })
}

/// `enclosed_name` đã bỏ `..` và gốc; ghép lại bằng `/` cho `SafeRoot`. Rỗng thì không có tên.
fn slash_path(path: &Path) -> Option<String> {
    let parts: Option<Vec<&str>> = path
        .components()
        .map(|part| match part {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect();
    let joined = parts?.join("/");
    (!joined.is_empty()).then_some(joined)
}

/// Giải nén theo `plan`. Lỗi giữa chừng thì giữ các file đã ghi (spec trang Tệp, mục 3).
pub(crate) fn extract_sync(mut plan: ExtractPlan, progress: Progress<'_>) -> Result<(), AppError> {
    fs::create_dir_all(&plan.dest).map_err(|err| io_err(&plan.dest, err))?;
    let mut done = 0u64;
    for item in &plan.items {
        let Some(size) = item.size else {
            fs::create_dir_all(&item.target).map_err(|err| io_err(&item.target, err))?;
            continue;
        };
        if let Some(parent) = item.target.parent() {
            fs::create_dir_all(parent).map_err(|err| io_err(parent, err))?;
        }
        let mut entry = plan.archive.by_index(item.index).map_err(zip_err)?;
        let mut out = File::create_new(&item.target).map_err(|err| io_err(&item.target, err))?;
        let mut limited = (&mut entry).take(size);
        let mut buf = vec![0u8; CHUNK];
        loop {
            let n = limited.read(&mut buf).map_err(read_err)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])
                .map_err(|err| io_err(&item.target, err))?;
            done += n as u64;
            progress(done)?;
        }
        // Đọc tới hết để zip kiểm CRC; còn dữ liệu sau cỡ đã khai báo là zip bị sửa (zip bomb).
        if entry.read(&mut [0u8; 1]).map_err(read_err)? != 0 {
            return Err(invalid("mục trong zip lớn hơn cỡ đã khai báo"));
        }
    }
    Ok(())
}

fn file_name(path: &Path) -> Result<String, AppError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| invalid("tên file không hợp lệ"))
}

fn zip_err(err: zip::result::ZipError) -> AppError {
    match err {
        zip::result::ZipError::Io(err) => write_err(err),
        other => invalid(&format!("zip: {other}")),
    }
}

fn read_err(err: io::Error) -> AppError {
    invalid(&format!("đọc zip: {err}"))
}

/// Ghi zip lỗi: đĩa đầy, hoặc người tải xuống ngắt giữa chừng (kênh đóng).
fn write_err(err: io::Error) -> AppError {
    AppError::new(ErrorCode::Internal, format!("ghi zip: {err}"))
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
