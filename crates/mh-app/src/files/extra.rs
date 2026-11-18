//! Tính năng bổ sung cho trang Tệp: nhân bản, tính dung lượng, xem trước log/ảnh/NBT, upload thư mục.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use mh_core::ServerId;
use mh_platform::SafeRoot;
use mh_store::ServerStore;

use super::archive::make_dirs_inside;
use super::disk::{
    Node, ensure_parent_dir, join_user, joined_file, node_at, open_root, require_dir,
};
use super::{
    AppError, child_path, exists, invalid, io_err, not_text, too_large, validate_file_name,
};

pub const PREVIEW_MAX_BYTES: u64 = 128 * 1024;
const NBT_MAX_BYTES: u64 = 4 * 1024 * 1024;
const DIR_SIZE_MAX_VISITED: usize = 200_000;
const COPY_MAX_ENTRIES: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirSize {
    pub bytes: u64,
    pub files: u64,
    pub dirs: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextPreview {
    pub text: String,
    pub total_bytes: u64,
    pub shown_bytes: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NbtPreview {
    pub json: String,
    pub size: u64,
    pub compressed: bool,
}

pub(super) fn dir_size_sync(root: &SafeRoot, user_path: &str) -> Result<DirSize, AppError> {
    let dir = join_user(root, user_path)?;
    require_dir(dir.as_path(), user_path)?;
    let mut stack = vec![dir.into_path_buf()];
    let (mut bytes, mut files, mut dirs, mut visited) = (0u64, 0u64, 0u64, 0usize);
    while let Some(current) = stack.pop() {
        let Ok(read) = fs::read_dir(&current) else {
            continue;
        };
        for entry in read.flatten() {
            visited += 1;
            if visited > DIR_SIZE_MAX_VISITED {
                return Ok(DirSize {
                    bytes,
                    files,
                    dirs,
                    truncated: true,
                });
            }
            let Ok(meta) = entry.metadata() else { continue };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                dirs += 1;
                stack.push(entry.path());
            } else if meta.is_file() {
                files += 1;
                bytes = bytes.saturating_add(meta.len());
            }
        }
    }
    Ok(DirSize {
        bytes,
        files,
        dirs,
        truncated: false,
    })
}

pub(super) fn copy_sync(root: &SafeRoot, from: &str, to: &str) -> Result<(), AppError> {
    let src = joined_file(root, from)?;
    let dest = joined_file(root, to)?;
    let node = node_at(src.as_path())?;
    if node == Node::Missing {
        return Err(AppError::not_found(from));
    }
    if node_at(dest.as_path())? != Node::Missing {
        return Err(exists(to));
    }
    if dest.as_path().starts_with(src.as_path()) {
        return Err(invalid("không chép thư mục vào chính nó"));
    }
    ensure_parent_dir(dest.as_path())?;
    if node == Node::File {
        fs::copy(src.as_path(), dest.as_path()).map_err(|err| io_err(dest.as_path(), err))?;
        return Ok(());
    }
    let mut count = 0usize;
    let res = copy_dir_tree(src.as_path(), dest.as_path(), &mut count);
    if res.is_err() {
        let _ = fs::remove_dir_all(dest.as_path());
    }
    res
}

fn copy_dir_tree(src: &Path, dest: &Path, count: &mut usize) -> Result<(), AppError> {
    fs::create_dir(dest).map_err(|err| io_err(dest, err))?;
    for entry in fs::read_dir(src).map_err(|err| io_err(src, err))? {
        let entry = entry.map_err(|err| io_err(src, err))?;
        *count += 1;
        if *count > COPY_MAX_ENTRIES {
            return Err(invalid("thư mục có quá nhiều mục để nhân bản"));
        }
        let meta = entry.metadata().map_err(|err| io_err(&entry.path(), err))?;
        if meta.file_type().is_symlink() {
            continue;
        }
        let target = dest.join(entry.file_name());
        if meta.is_dir() {
            copy_dir_tree(&entry.path(), &target, count)?;
        } else if meta.is_file() {
            fs::copy(entry.path(), &target).map_err(|err| io_err(&target, err))?;
        }
    }
    Ok(())
}

pub(super) fn read_preview_sync(root: &SafeRoot, user_path: &str) -> Result<TextPreview, AppError> {
    let path = join_user(root, user_path)?;
    match node_at(path.as_path())? {
        Node::File => {}
        Node::Dir => return Err(invalid("không phải file")),
        Node::Missing => return Err(AppError::not_found(user_path)),
    }
    let mut file = File::open(path.as_path()).map_err(|err| io_err(path.as_path(), err))?;
    let total_bytes = file
        .metadata()
        .map_err(|err| io_err(path.as_path(), err))?
        .len();
    let truncated = total_bytes > PREVIEW_MAX_BYTES;
    if truncated {
        file.seek(SeekFrom::Start(total_bytes - PREVIEW_MAX_BYTES))
            .map_err(|err| io_err(path.as_path(), err))?;
    }
    let mut buf = Vec::new();
    file.take(PREVIEW_MAX_BYTES)
        .read_to_end(&mut buf)
        .map_err(|err| io_err(path.as_path(), err))?;
    let slice = align_utf8_tail(&buf, truncated);
    let text = std::str::from_utf8(slice)
        .map_err(|_| not_text())?
        .to_owned();
    if text.contains('\0') {
        return Err(not_text());
    }
    let shown_bytes = text.len() as u64;
    Ok(TextPreview {
        text,
        total_bytes,
        shown_bytes,
        truncated,
    })
}

fn align_utf8_tail(buf: &[u8], truncated: bool) -> &[u8] {
    if !truncated {
        return buf;
    }
    let mut start = 0;
    while start < buf.len() && start < 3 && (buf[start] & 0b1100_0000) == 0b1000_0000 {
        start += 1;
    }
    let aligned = &buf[start..];
    if let Some(pos) = aligned.iter().position(|&b| b == b'\n') {
        &aligned[pos + 1..]
    } else {
        aligned
    }
}

pub(super) fn read_nbt_sync(root: &SafeRoot, user_path: &str) -> Result<NbtPreview, AppError> {
    let path = join_user(root, user_path)?;
    match node_at(path.as_path())? {
        Node::File => {}
        Node::Dir => return Err(invalid("không phải file")),
        Node::Missing => return Err(AppError::not_found(user_path)),
    }
    let size = fs::metadata(path.as_path())
        .map_err(|err| io_err(path.as_path(), err))?
        .len();
    if size > NBT_MAX_BYTES {
        return Err(too_large(size));
    }
    let bytes = fs::read(path.as_path()).map_err(|err| io_err(path.as_path(), err))?;
    let compressed = bytes.starts_with(&[0x1f, 0x8b]);
    let raw = if compressed {
        let mut out = Vec::new();
        GzDecoder::new(bytes.as_slice())
            .take(NBT_MAX_BYTES + 1)
            .read_to_end(&mut out)
            .map_err(|_| invalid("không giải nén được file NBT"))?;
        if out.len() as u64 > NBT_MAX_BYTES {
            return Err(too_large(out.len() as u64));
        }
        out
    } else {
        bytes
    };
    let value: fastnbt::Value =
        fastnbt::from_bytes(&raw).map_err(|_| invalid("không đọc được dữ liệu NBT"))?;
    let json = serde_json::to_string_pretty(&value)
        .map_err(|err| invalid(&format!("chuyển NBT sang JSON: {err}")))?;
    Ok(NbtPreview {
        json,
        size,
        compressed,
    })
}

pub(super) fn open_image_sync(
    store: &ServerStore,
    id: ServerId,
    user_path: &str,
) -> Result<(String, &'static str, File), AppError> {
    let mime = image_mime(user_path).ok_or_else(|| invalid("không phải định dạng ảnh hỗ trợ"))?;
    let root = open_root(store, id)?;
    let path = joined_file(&root, user_path)?;
    match node_at(path.as_path())? {
        Node::File => {}
        Node::Dir => return Err(invalid("không phải file")),
        Node::Missing => return Err(AppError::not_found(user_path)),
    }
    let name = path
        .as_path()
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| invalid("tên file không hợp lệ"))?
        .to_owned();
    let file = File::open(path.as_path()).map_err(|err| io_err(path.as_path(), err))?;
    Ok((name, mime, file))
}

fn image_mime(path: &str) -> Option<&'static str> {
    let ext = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

pub(super) fn normalize_relative_path(raw: &str) -> Result<String, AppError> {
    let parts: Vec<&str> = raw.split('/').collect();
    if parts.is_empty() {
        return Err(invalid("tên file không hợp lệ"));
    }
    for part in &parts {
        validate_file_name(part)?;
    }
    Ok(parts.join("/"))
}

pub(super) fn upload_relative_paths(
    store: &ServerStore,
    id: ServerId,
    dir: &str,
    rel_path: &str,
) -> Result<(PathBuf, PathBuf), AppError> {
    let root = open_root(store, id)?;
    let dir_path = join_user(&root, dir)?;
    require_dir(dir_path.as_path(), dir)?;
    let rel = child_path(dir, rel_path);
    let dest = join_user(&root, &rel)?;
    if node_at(dest.as_path())? != Node::Missing {
        return Err(exists(&rel));
    }
    if let Some(parent) = dest.as_path().parent() {
        let real_root = fs::canonicalize(root.path()).map_err(|err| io_err(root.path(), err))?;
        make_dirs_inside(&real_root, parent)?;
    }
    let mut part = dest.as_path().as_os_str().to_os_string();
    part.push(".part");
    Ok((dest.into_path_buf(), PathBuf::from(part)))
}
