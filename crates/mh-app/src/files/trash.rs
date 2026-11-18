//! Quản lý thùng rác của trang Tệp: liệt kê, khôi phục và dọn sạch `trash/<id>/files/`.

use std::fs;
use std::path::{Path, PathBuf};

use mh_core::ServerId;
use mh_platform::SafeRoot;
use mh_store::ServerStore;
use time::OffsetDateTime;

use super::archive::make_dirs_inside;
use super::disk::{Node, join_user, node_at, open_root};
use super::{AppError, exists, invalid, io_err, validate_file_name};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashBatch {
    pub id: String,
    pub deleted_at: Option<OffsetDateTime>,
    pub items: Vec<String>,
    pub bytes: u64,
}

pub(super) fn list_trash_sync(
    trash_root: &Path,
    id: ServerId,
) -> Result<Vec<TrashBatch>, AppError> {
    let base = trash_base(trash_root, id);
    if !base.is_dir() {
        return Ok(Vec::new());
    }
    let mut batches = Vec::new();
    for entry in fs::read_dir(&base).map_err(|err| io_err(&base, err))? {
        let entry = entry.map_err(|err| io_err(&base, err))?;
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().into_string().ok() else {
            continue;
        };
        let (items, bytes) = summarize_batch(&entry.path())?;
        if items.is_empty() {
            continue;
        }
        batches.push(TrashBatch {
            id: name,
            deleted_at: meta.modified().ok().map(OffsetDateTime::from),
            items,
            bytes,
        });
    }
    batches.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(batches)
}

pub(super) fn restore_trash_sync(
    store: &ServerStore,
    trash_root: &Path,
    id: ServerId,
    batch: &str,
) -> Result<Vec<String>, AppError> {
    validate_file_name(batch)?;
    let batch_dir = trash_base(trash_root, id).join(batch);
    if !batch_dir.is_dir() {
        return Err(AppError::not_found(batch));
    }
    let root = open_root(store, id)?;
    let mut moves = Vec::new();
    collect_restore_moves(&batch_dir, &batch_dir, &root, &mut moves)?;
    if moves.is_empty() {
        return Err(AppError::not_found(batch));
    }
    let real_root = fs::canonicalize(root.path()).map_err(|err| io_err(root.path(), err))?;
    let mut restored = Vec::with_capacity(moves.len());
    for (src, dest, rel) in moves {
        if let Some(parent) = dest.parent() {
            make_dirs_inside(&real_root, parent)?;
        }
        fs::rename(&src, &dest).map_err(|err| io_err(&src, err))?;
        restored.push(rel);
    }
    let _ = fs::remove_dir_all(&batch_dir);
    Ok(restored)
}

pub(super) fn empty_trash_sync(
    trash_root: &Path,
    id: ServerId,
    batch: Option<&str>,
) -> Result<(), AppError> {
    let base = trash_base(trash_root, id);
    if let Some(batch) = batch {
        validate_file_name(batch)?;
        let target = base.join(batch);
        if !target.exists() {
            return Err(AppError::not_found(batch));
        }
        return fs::remove_dir_all(&target).map_err(|err| io_err(&target, err));
    }
    if base.exists() {
        fs::remove_dir_all(&base).map_err(|err| io_err(&base, err))?;
    }
    Ok(())
}

pub(super) fn trash_batch_has_server_jar(trash_root: &Path, id: ServerId, batch: &str) -> bool {
    validate_file_name(batch).is_ok()
        && trash_base(trash_root, id)
            .join(batch)
            .join("server.jar")
            .exists()
}

fn trash_base(trash_root: &Path, id: ServerId) -> PathBuf {
    trash_root.join(id.to_string()).join("files")
}

fn summarize_batch(batch_dir: &Path) -> Result<(Vec<String>, u64), AppError> {
    let mut items = Vec::new();
    let mut bytes = 0u64;
    let mut stack = vec![batch_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|err| io_err(&dir, err))? {
            let entry = entry.map_err(|err| io_err(&dir, err))?;
            let meta = entry.metadata().map_err(|err| io_err(&entry.path(), err))?;
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                bytes = bytes.saturating_add(meta.len());
                if let Ok(rel) = entry.path().strip_prefix(batch_dir) {
                    items.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    if items.is_empty() {
        // Thư mục rỗng còn sót lại: vẫn tính tên cấp một để hiện được.
        for entry in fs::read_dir(batch_dir).map_err(|err| io_err(batch_dir, err))? {
            let entry = entry.map_err(|err| io_err(batch_dir, err))?;
            if let Ok(rel) = entry.path().strip_prefix(batch_dir) {
                items.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    items.sort();
    Ok((items, bytes))
}

fn collect_restore_moves(
    batch_dir: &Path,
    current: &Path,
    root: &SafeRoot,
    out: &mut Vec<(PathBuf, PathBuf, String)>,
) -> Result<(), AppError> {
    for entry in fs::read_dir(current).map_err(|err| io_err(current, err))? {
        let entry = entry.map_err(|err| io_err(current, err))?;
        let path = entry.path();
        let rel_path = path
            .strip_prefix(batch_dir)
            .map_err(|_| invalid("đường dẫn thùng rác không hợp lệ"))?;
        let rel = rel_path.to_string_lossy().replace('\\', "/");
        let dest = join_user(root, &rel)?;
        let node = node_at(dest.as_path())?;
        let meta = entry.metadata().map_err(|err| io_err(&path, err))?;
        if meta.is_dir() {
            match node {
                Node::Missing => out.push((path, dest.into_path_buf(), rel)),
                Node::Dir => collect_restore_moves(batch_dir, &path, root, out)?,
                Node::File => return Err(exists(&rel)),
            }
        } else {
            if node != Node::Missing {
                return Err(exists(&rel));
            }
            out.push((path, dest.into_path_buf(), rel));
        }
    }
    Ok(())
}
