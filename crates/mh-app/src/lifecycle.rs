//! Khởi động và tắt controller (spec P1, mục 5.6 "Dấu đang chạy" và 5.7).

use mh_core::ServerState;
use tokio::task::JoinSet;

use crate::{App, AppError};

impl App {
    /// Gọi một lần lúc controller khởi động: bật lại server còn dấu `.running` (controller đã
    /// chết giữa chừng) và server có `autostart`. Trả số server được bật.
    pub async fn reconcile(&self) -> Result<usize, AppError> {
        let mut started = 0;
        for listed in self.store.list()? {
            let spec = match listed.spec {
                Ok(spec) => spec,
                Err(reason) => {
                    tracing::warn!(server = %listed.id, %reason, "bỏ qua server hỏng");
                    continue;
                }
            };
            let handle = self.supervisor(spec.id);
            if !(listed.marked_running || spec.policy.autostart) {
                continue;
            }
            match handle.start().await {
                Ok(()) => started += 1,
                Err(e) => {
                    tracing::warn!(server = %spec.id, error = %e, "không bật lại được server");
                    if let Err(e) = self.store.clear_running(spec.id) {
                        tracing::warn!(server = %spec.id, error = %e, "không xóa được dấu .running");
                    }
                }
            }
        }
        Ok(started)
    }

    /// Dừng an toàn mọi server cùng lúc; gọi `on_progress(xong, tổng)` sau mỗi server.
    /// Server nào quá hạn `stop` thì supervisor tự kill.
    pub async fn shutdown_all(&self, mut on_progress: impl FnMut(usize, usize)) {
        let running: Vec<_> = self
            .supervisor_handles()
            .into_iter()
            .filter(|(_, h)| {
                let status = h.status();
                status.state.has_process() || status.restart_pending
            })
            .collect();
        let total = running.len();
        on_progress(0, total);
        let timeout = self.shutdown_timeout();
        let mut tasks = JoinSet::new();
        for (id, handle) in running {
            tasks.spawn(async move { (id, handle.stop_and_wait(timeout).await) });
        }
        let mut done = 0;
        while let Some(result) = tasks.join_next().await {
            done += 1;
            match result {
                Ok((id, status)) if status.state != ServerState::Stopped => {
                    tracing::warn!(server = %id, state = ?status.state, "server không dừng sạch");
                }
                Ok(_) => {}
                Err(e) => tracing::error!(error = %e, "task dừng server lỗi"),
            }
            on_progress(done, total);
        }
    }
}
