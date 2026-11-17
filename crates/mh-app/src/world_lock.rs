//! Mỗi server chỉ một thao tác chạm cả world tại một lúc: sao lưu, khôi phục, cập nhật mod, cập nhật
//! modpack, đổi phiên bản (spec P2, mục 5.7). Hai việc chồng nhau thì một bản backup có thể chứa
//! world đang bị thay giữa chừng mà vẫn mang hash hợp lệ, và chỉ lộ ra lúc khôi phục.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mh_core::{ErrorCode, ServerId};

use crate::jobs::JobKind;
use crate::{App, AppError, Status};

pub(crate) type WorldJobs = Arc<Mutex<HashMap<ServerId, JobKind>>>;

/// Giữ suốt Job. Drop (kể cả khi Job panic hay bị hủy) là nhả cho thao tác sau.
#[derive(Debug)]
pub(crate) struct WorldGuard {
    jobs: WorldJobs,
    id: ServerId,
}

impl Drop for WorldGuard {
    fn drop(&mut self) {
        lock(&self.jobs).remove(&self.id);
    }
}

impl App {
    /// Gọi trước khi tạo Job, để thao tác thứ hai bị từ chối ngay chứ không chờ trong hàng.
    pub(crate) fn claim_world(&self, id: ServerId, kind: JobKind) -> Result<WorldGuard, AppError> {
        let mut jobs = lock(&self.world_jobs);
        if let Some(running) = jobs.get(&id) {
            return Err(world_busy(*running));
        }
        jobs.insert(id, kind);
        Ok(WorldGuard {
            jobs: Arc::clone(&self.world_jobs),
            id,
        })
    }

    /// Bật hoặc khởi động lại giữa lúc đang sao lưu, khôi phục, cập nhật sẽ ghi vào world đang
    /// được nén hoặc thay.
    pub(crate) fn ensure_world_idle(&self, id: ServerId) -> Result<(), AppError> {
        match lock(&self.world_jobs).get(&id) {
            Some(running) => Err(world_busy(*running)),
            None => Ok(()),
        }
    }
}

/// Process của server có đổi giữa hai ảnh trạng thái không: bật, tắt, crash rồi tự bật lại. Đang
/// `Stopping` cũng tính là đổi vì lệnh `stop` lưu world bất kể `save-off`.
pub(crate) fn process_changed(before: &Status, after: &Status) -> bool {
    before.state != after.state || before.pid != after.pid
}

fn world_busy(running: JobKind) -> AppError {
    let job = serde_json::to_value(running).unwrap_or_default();
    AppError::new(
        ErrorCode::ServerBusy,
        format!("server đang có Job {job} chạm vào world"),
    )
    .with_params(serde_json::json!({ "reason": "world_job", "job": job }))
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // Chỉ bị poison khi một luồng panic lúc đang giữ; dữ liệu bên trong vẫn dùng được.
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use mh_core::ServerState;

    use super::*;

    fn status(state: ServerState, pid: Option<u32>) -> Status {
        Status {
            state,
            pid,
            running_since: None,
            crash: None,
            restart_pending: false,
            slow_start: false,
        }
    }

    #[test]
    fn restart_with_new_pid_counts_as_changed() {
        let before = status(ServerState::Running, Some(10));
        assert!(!process_changed(
            &before,
            &status(ServerState::Running, Some(10))
        ));
        assert!(process_changed(
            &before,
            &status(ServerState::Running, Some(11))
        ));
        assert!(process_changed(
            &before,
            &status(ServerState::Stopping, Some(10))
        ));
        let off = status(ServerState::Stopped, None);
        assert!(process_changed(
            &off,
            &status(ServerState::Starting, Some(3))
        ));
        assert!(!process_changed(&off, &status(ServerState::Stopped, None)));
    }
}
