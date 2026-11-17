//! Tìm file theo tên trong một thư mục và mọi thư mục con (spec trang Tệp, mục 3).

use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

use mh_platform::SafeRoot;
use time::OffsetDateTime;

use super::disk::{join_user, require_dir};
use super::{AppError, FileKind, child_path, invalid, io_err};

/// Số kết quả tối đa trả về.
pub const SEARCH_MAX_RESULTS: usize = 200;
/// Số mục tối đa được duyệt, để world lớn không làm route treo lâu.
pub const SEARCH_MAX_VISITED: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundEntry {
    /// Đường dẫn tính từ gốc server, ngăn bằng `/`.
    pub path: String,
    pub kind: FileKind,
    pub size: Option<u64>,
    pub modified: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub items: Vec<FoundEntry>,
    /// Có thì còn kết quả chưa trả (chạm một trong hai giới hạn trên).
    pub truncated: bool,
}

/// Duyệt theo chiều rộng nên kết quả nông hơn đứng trước. Không đi theo symlink.
pub(super) fn search_sync(
    root: &SafeRoot,
    start: &str,
    query: &str,
    limits: (usize, usize),
) -> Result<SearchResult, AppError> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Err(invalid("chuỗi tìm rỗng"));
    }
    let (max_results, max_visited) = limits;
    let dir = join_user(root, start)?;
    require_dir(dir.as_path(), start)?;
    let mut queue = VecDeque::from([(dir.into_path_buf(), start.trim_matches('/').to_owned())]);
    let mut items = Vec::new();
    let mut visited = 0usize;
    while let Some((abs, rel)) = queue.pop_front() {
        let Ok(read) = fs::read_dir(&abs) else {
            // Thư mục không đọc được (quyền, vừa bị xóa): bỏ qua, không hỏng cả lượt tìm.
            continue;
        };
        for entry in read {
            let entry = entry.map_err(|err| io_err(&abs, err))?;
            visited += 1;
            if visited > max_visited || items.len() >= max_results {
                return Ok(SearchResult {
                    items,
                    truncated: true,
                });
            }
            let Some(found) = visit(&entry, &rel, &needle, &mut queue) else {
                continue;
            };
            items.push(found);
        }
    }
    Ok(SearchResult {
        items,
        truncated: false,
    })
}

/// Đưa thư mục con vào hàng đợi; trả mục nếu tên khớp.
fn visit(
    entry: &fs::DirEntry,
    rel_dir: &str,
    needle: &str,
    queue: &mut VecDeque<(PathBuf, String)>,
) -> Option<FoundEntry> {
    let name = entry.file_name().into_string().ok()?;
    // `DirEntry::metadata` không đi theo symlink: link không bị duyệt và không ra kết quả.
    let meta = entry.metadata().ok()?;
    if meta.file_type().is_symlink() {
        return None;
    }
    let path = child_path(rel_dir, &name);
    let is_dir = meta.is_dir();
    if is_dir {
        queue.push_back((entry.path(), path.clone()));
    }
    if !name.to_lowercase().contains(needle) {
        return None;
    }
    Some(FoundEntry {
        path,
        kind: if is_dir {
            FileKind::Dir
        } else {
            FileKind::File
        },
        size: if is_dir { None } else { Some(meta.len()) },
        modified: meta.modified().ok().map(OffsetDateTime::from),
    })
}
