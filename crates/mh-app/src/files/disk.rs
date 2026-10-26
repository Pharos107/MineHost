//! Thao tác đĩa đồng bộ của dịch vụ tệp. Chạy trong `spawn_blocking`.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use mh_core::ServerId;
use mh_platform::fs::write_atomic;
use mh_platform::{SafePath, SafeRoot};
use mh_store::ServerStore;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use super::{
    AppError, FileEntry, FileKind, TEXT_LIMIT, TextFile, changed, child_path, exists, invalid,
    io_err, not_text, plat, too_large,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Node {
    Missing,
    File,
    Dir,
}

pub(super) fn list_dir_sync(root: &SafeRoot, user_path: &str) -> Result<Vec<FileEntry>, AppError> {
    let dir = join_user(root, user_path)?;
    require_dir(dir.as_path(), user_path)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir.as_path()).map_err(|err| io_err(dir.as_path(), err))? {
        let entry = entry.map_err(|err| io_err(dir.as_path(), err))?;
        if let Some(item) = list_item(root, user_path, &entry)? {
            entries.push(item);
        }
    }
    entries.sort_by(cmp_entry);
    Ok(entries)
}

pub(super) fn read_text_sync(root: &SafeRoot, user_path: &str) -> Result<TextFile, AppError> {
    let path = join_user(root, user_path)?;
    match node_at(path.as_path())? {
        Node::File => {}
        Node::Dir => return Err(invalid("không phải file")),
        Node::Missing => return Err(AppError::not_found(user_path)),
    }
    let bytes = read_limited(path.as_path())?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| not_text())?
        .to_owned();
    if text.contains('\0') {
        return Err(not_text());
    }
    Ok(TextFile {
        etag: content_etag(&bytes),
        size: bytes.len() as u64,
        text,
    })
}

pub(super) fn write_text_sync(
    root: &SafeRoot,
    user_path: &str,
    text: &str,
    if_match: Option<&str>,
) -> Result<String, AppError> {
    if text.len() as u64 > TEXT_LIMIT {
        return Err(too_large(text.len() as u64));
    }
    if text.contains('\0') {
        return Err(not_text());
    }
    let path = joined_file(root, user_path)?;
    match node_at(path.as_path())? {
        Node::Dir => return Err(exists(user_path)),
        Node::File => ensure_etag(path.as_path(), if_match)?,
        Node::Missing => {
            if if_match.is_some() {
                return Err(changed());
            }
            ensure_parent_dir(path.as_path())?;
        }
    }
    let bytes = text.as_bytes();
    write_atomic(path.as_path(), bytes).map_err(plat)?;
    Ok(content_etag(bytes))
}

pub(super) fn create_dir_sync(root: &SafeRoot, user_path: &str) -> Result<(), AppError> {
    let path = join_user(root, user_path)?;
    // Gốc đã có sẵn. Tạo lại gốc cũng là `FILE_EXISTS`, không phải sửa thư mục server.
    if path.as_path() == root.path() || node_at(path.as_path())? != Node::Missing {
        return Err(exists(user_path));
    }
    ensure_parent_dir(path.as_path())?;
    fs::create_dir(path.as_path()).map_err(|err| io_err(path.as_path(), err))
}

pub(super) fn rename_sync(root: &SafeRoot, from: &str, to: &str) -> Result<(), AppError> {
    let src = joined_file(root, from)?;
    let dest = joined_file(root, to)?;
    if node_at(src.as_path())? == Node::Missing {
        return Err(AppError::not_found(from));
    }
    if node_at(dest.as_path())? != Node::Missing {
        return Err(exists(to));
    }
    ensure_parent_dir(dest.as_path())?;
    fs::rename(src.as_path(), dest.as_path()).map_err(|err| io_err(src.as_path(), err))
}

pub(super) fn delete_sync(
    store: &ServerStore,
    trash: &Path,
    id: ServerId,
    user_path: &str,
) -> Result<PathBuf, AppError> {
    let root = open_root(store, id)?;
    let src = join_user(&root, user_path)?;
    let rel = relative_inside(&root, src.as_path())?;
    if node_at(src.as_path())? == Node::Missing {
        return Err(AppError::not_found(user_path));
    }
    let dest = trash_destination(trash, id, &rel)?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|err| io_err(parent, err))?;
    }
    fs::rename(src.as_path(), &dest).map_err(|err| io_err(src.as_path(), err))?;
    tracing::info!(server = %id, dest = %dest.display(), "đã chuyển file vào thùng rác");
    Ok(dest)
}

pub(super) fn upload_paths(
    store: &ServerStore,
    id: ServerId,
    dir: &str,
    file_name: &str,
) -> Result<(PathBuf, PathBuf), AppError> {
    let root = open_root(store, id)?;
    let dir_path = join_user(&root, dir)?;
    require_dir(dir_path.as_path(), dir)?;
    let rel = child_path(dir, file_name);
    let dest = join_user(&root, &rel)?;
    if node_at(dest.as_path())? != Node::Missing {
        return Err(exists(&rel));
    }
    let mut part = dest.as_path().as_os_str().to_owned();
    part.push(".part");
    Ok((dest.into_path_buf(), PathBuf::from(part)))
}

pub(super) fn open_root(store: &ServerStore, id: ServerId) -> Result<SafeRoot, AppError> {
    store.load(id)?;
    let dir = store.paths(id).server_dir;
    if !dir.is_dir() {
        return Err(AppError::not_found(dir.display()));
    }
    Ok(SafeRoot::new(dir)?)
}

fn list_item(
    root: &SafeRoot,
    dir: &str,
    entry: &fs::DirEntry,
) -> Result<Option<FileEntry>, AppError> {
    let Some(name) = entry.file_name().into_string().ok() else {
        tracing::debug!("bỏ qua tên file không phải UTF-8");
        return Ok(None);
    };
    let path = match join_user(root, &child_path(dir, &name)) {
        Ok(path) => path,
        // Link trỏ ra ngoài, hoặc tên OS không chấp nhận: bỏ qua, không hỏng cả thư mục.
        Err(err) if super::skips_list_entry(&err) => {
            tracing::debug!(name, code = %err.code, "bỏ qua mục không an toàn");
            return Ok(None);
        }
        Err(err) => return Err(err),
    };
    let meta = match fs::metadata(path.as_path()) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(io_err(path.as_path(), err)),
    };
    let is_dir = meta.is_dir();
    Ok(Some(FileEntry {
        name,
        kind: if is_dir {
            FileKind::Dir
        } else {
            FileKind::File
        },
        size: if is_dir { None } else { Some(meta.len()) },
        modified: meta.modified().ok().map(OffsetDateTime::from),
    }))
}

fn join_user(root: &SafeRoot, user_path: &str) -> Result<SafePath, AppError> {
    // SafeRoot hiểu `/...` là tính từ gốc. Tuyệt đối của OS (`/etc`, `C:\`) phải chặn trước.
    if Path::new(user_path).is_absolute() {
        return Err(super::outside(user_path));
    }
    Ok(root.join(user_path)?)
}

/// Từ chối chính thư mục `server/`: đổi tên hoặc ghi đè gốc làm hỏng server.
fn joined_file(root: &SafeRoot, user_path: &str) -> Result<SafePath, AppError> {
    let path = join_user(root, user_path)?;
    if path.as_path() == root.path() {
        return Err(invalid("không sửa thư mục gốc của server"));
    }
    Ok(path)
}

fn relative_inside(root: &SafeRoot, path: &Path) -> Result<PathBuf, AppError> {
    match path.strip_prefix(root.path()) {
        Ok(rel) if !rel.as_os_str().is_empty() => Ok(rel.to_path_buf()),
        _ => Err(invalid("không xóa thư mục gốc của server")),
    }
}

fn require_dir(path: &Path, user_path: &str) -> Result<(), AppError> {
    match node_at(path)? {
        Node::Dir => Ok(()),
        Node::File => Err(invalid("không phải thư mục")),
        Node::Missing => Err(AppError::not_found(user_path)),
    }
}

fn node_at(path: &Path) -> Result<Node, AppError> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Node::Missing),
        Err(err) => return Err(io_err(path, err)),
    };
    if !meta.file_type().is_symlink() {
        return Ok(if meta.is_dir() { Node::Dir } else { Node::File });
    }
    // Link gãy vẫn là một mục: xóa và đổi tên được, không coi là mất file.
    match fs::metadata(path) {
        Ok(meta) if meta.is_dir() => Ok(Node::Dir),
        Ok(_) => Ok(Node::File),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Node::File),
        Err(err) => Err(io_err(path, err)),
    }
}

fn ensure_etag(path: &Path, if_match: Option<&str>) -> Result<(), AppError> {
    let current = file_etag(path)?;
    match if_match {
        Some(tag) if tag == current => Ok(()),
        _ => Err(changed()),
    }
}

fn file_etag(path: &Path) -> Result<String, AppError> {
    let mut file = File::open(path).map_err(|err| io_err(path, err))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|err| io_err(path, err))?;
        if n == 0 {
            return Ok(hex_encode(&hasher.finalize()));
        }
        hasher.update(&buf[..n]);
    }
}

fn read_limited(path: &Path) -> Result<Vec<u8>, AppError> {
    let meta = fs::metadata(path).map_err(|err| io_err(path, err))?;
    if meta.len() > TEXT_LIMIT {
        return Err(too_large(meta.len()));
    }
    let bytes = fs::read(path).map_err(|err| io_err(path, err))?;
    if bytes.len() as u64 > TEXT_LIMIT {
        return Err(too_large(bytes.len() as u64));
    }
    Ok(bytes)
}

fn ensure_parent_dir(path: &Path) -> Result<(), AppError> {
    match path.parent() {
        Some(parent) if parent.is_dir() => Ok(()),
        Some(parent) => Err(AppError::not_found(parent.display())),
        None => Err(invalid("không có thư mục cha")),
    }
}

fn trash_destination(trash: &Path, id: ServerId, rel: &Path) -> Result<PathBuf, AppError> {
    use mh_core::ErrorCode;
    let stamp = OffsetDateTime::now_utc()
        .to_offset(time::UtcOffset::UTC)
        .format(time::macros::format_description!(
            "[year][month][day]-[hour][minute][second]"
        ))
        .map_err(|err| AppError::new(ErrorCode::Internal, format!("thời điểm thùng rác: {err}")))?;
    let base = trash.join(id.to_string()).join("files");
    // Cùng giây, cùng đường dẫn: thêm hậu tố để không đè bản vừa bỏ vào thùng rác.
    let mut folder = base.join(&stamp);
    let mut n = 2u32;
    while folder.join(rel).exists() {
        folder = base.join(format!("{stamp}-{n}"));
        n += 1;
        if n > 1000 {
            return Err(AppError::new(
                ErrorCode::Internal,
                "quá nhiều bản trong cùng một giây",
            ));
        }
    }
    Ok(folder.join(rel))
}

fn content_etag(bytes: &[u8]) -> String {
    // Giữ đủ 64 ký tự hex. Rút còn 16 thì hai nội dung khác nhau có thể cùng etag.
    hex_encode(&Sha256::digest(bytes))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(*byte >> 4) as usize] as char);
        out.push(HEX[(*byte & 0x0f) as usize] as char);
    }
    out
}

fn cmp_entry(a: &FileEntry, b: &FileEntry) -> std::cmp::Ordering {
    let rank = |kind: FileKind| match kind {
        FileKind::Dir => 0,
        FileKind::File => 1,
    };
    rank(a.kind).cmp(&rank(b.kind)).then_with(|| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    })
}
