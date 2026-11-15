//! Bộ nhớ đệm tải về (spec P3, mục 3.4) và trạng thái giới hạn tài nguyên (mục 3.8) cho trang
//! Cài đặt. Cache không tự dọn trong P3; người dùng xóa bằng tay khi cần chỗ trống.

use std::path::Path;

use mh_core::ErrorCode;
use mh_net::CACHE_ALGOS;

use crate::jobs::{JobKind, JobState};
use crate::{App, AppError};

impl App {
    /// Tổng byte file trong `cache/<thuật toán>/`. Đọc cây thư mục: gọi trong `spawn_blocking`.
    pub fn download_cache_size(&self) -> u64 {
        let root = self.dirs.cache();
        CACHE_ALGOS
            .iter()
            .map(|algo| tree_size(&root.join(algo)))
            .sum()
    }

    /// Xóa file tải về theo hash, trả số byte đã giải phóng. Có Job đang tải thì từ chối: Job đó
    /// có thể đang chép file từ cache sang thư mục server.
    pub fn clear_download_cache(&self) -> Result<u64, AppError> {
        let busy = self
            .jobs
            .list(None)
            .iter()
            .any(|job| job.state == JobState::Running && downloads(job.kind));
        if busy {
            return Err(AppError::new(
                ErrorCode::ServerBusy,
                "đang có Job tải file, thử lại khi Job xong",
            ));
        }
        let root = self.dirs.cache();
        let mut freed = 0u64;
        for algo in CACHE_ALGOS {
            let dir = root.join(algo);
            let size = tree_size(&dir);
            match std::fs::remove_dir_all(&dir) {
                Ok(()) => freed = freed.saturating_add(size),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(AppError::new(
                        ErrorCode::Internal,
                        format!("không xóa được {}: {err}", dir.display()),
                    ));
                }
            }
        }
        tracing::info!(bytes = freed, "đã xóa bộ nhớ đệm tải về");
        Ok(freed)
    }

    /// Máy này có giới hạn được RAM và số process của từng server không (Job Object trên
    /// Windows, cgroup v2 ghi được trên Linux).
    pub fn resource_limits_enforced(&self) -> bool {
        self.runtime.limits_enforced()
    }
}

/// Job có tải file vào cache. Sao lưu, khôi phục và kiểm tra cập nhật thì không.
fn downloads(kind: JobKind) -> bool {
    !matches!(
        kind,
        JobKind::Backup | JobKind::Restore | JobKind::CheckUpdates
    )
}

fn tree_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    total
}
