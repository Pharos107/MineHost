//! Khởi động và tắt controller (spec P1, mục 5.6 "Dấu đang chạy" và 5.7).

use std::time::Duration;

use mh_core::{ErrorCode, ServerState};
use mh_supervisor::SupervisorError;
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::{App, AppError, SupervisorHandle};

impl App {
    /// Gọi một lần lúc controller khởi động: bật lại server còn dấu `.running` (controller đã
    /// chết giữa chừng) và server có `autostart`. Trả số server được bật.
    ///
    /// Các server bật song song, nên dù nhiều server phải chờ cổng thì tổng thời gian chờ vẫn
    /// không quá `PORT_RELEASE_WAIT`.
    pub async fn reconcile(&self) -> Result<usize, AppError> {
        let deadline = Instant::now() + PORT_RELEASE_WAIT;
        let mut tasks = JoinSet::new();
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
            // Chỉ server còn dấu `.running` mới có Java cũ có thể đang giữ cổng.
            let wait_until = listed.marked_running.then_some(deadline);
            tasks.spawn(async move { (spec.id, start_waiting_port(&handle, wait_until).await) });
        }
        let mut started = 0;
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok((_, Ok(()))) => started += 1,
                Ok((id, Err(e))) => {
                    tracing::warn!(server = %id, error = %e, "không bật lại được server");
                    if let Err(e) = self.store.clear_running(id) {
                        tracing::warn!(server = %id, error = %e, "không xóa được dấu .running");
                    }
                }
                Err(e) => tracing::error!(error = %e, "task bật lại server lỗi"),
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

/// Java của lần chạy trước có thể chưa thoát hẳn khi controller mới đã lên: luồng chính đã chết
/// nhưng các luồng khác còn giữ socket thêm vài chục mili giây, lâu hơn nếu heap lớn và máy bận.
const PORT_RELEASE_WAIT: Duration = Duration::from_secs(10);
const PORT_RETRY_EVERY: Duration = Duration::from_millis(250);

/// Bật server; có `wait_until` thì khi cổng còn bận, chờ process cũ nhả cổng rồi thử lại.
async fn start_waiting_port(
    handle: &SupervisorHandle,
    wait_until: Option<Instant>,
) -> Result<(), SupervisorError> {
    loop {
        match handle.start().await {
            Err(e)
                if e.code() == ErrorCode::PortInUse
                    && wait_until.is_some_and(|t| Instant::now() < t) =>
            {
                tokio::time::sleep(PORT_RETRY_EVERY).await;
            }
            result => return result,
        }
    }
}
