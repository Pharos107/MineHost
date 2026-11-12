//! Sao lưu tự động theo lịch (spec P2, mục 5.7).
//!
//! Một task kiểm tra mỗi phút. Lần kiểm tra đầu chờ 5 phút sau khi controller bật, để không sao
//! lưu ngay lúc mọi server đang khởi động. Server không chạy kể từ bản trước thì bỏ qua: world
//! không đổi, sao lưu nữa chỉ tốn chỗ. Xóa bản cũ vượt `keep` nằm ở cuối Job sao lưu.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};
use std::time::Duration;

use mh_core::{BackupSchedule, ServerId, ServerState, next_run};

use crate::App;
use crate::backup::{BackupKind, BackupRequest};
use crate::restart_schedule::local_now;

const FIRST_DELAY: Duration = Duration::from_secs(5 * 60);
const TICK: Duration = Duration::from_secs(60);
/// Job đã chạy mà chưa thấy bản mới quá chừng này (Job lỗi) thì cho thử lại.
const RETRY_AFTER_SECS: i64 = 60 * 60;

/// Lịch của một server tại một lượt kiểm tra.
#[derive(Debug, Clone)]
pub struct BackupCandidate {
    pub id: ServerId,
    pub schedule: BackupSchedule,
    /// Thời điểm bản `scheduled` gần nhất, giây Unix.
    pub last: Option<i64>,
    /// Server đang có process.
    pub running: bool,
}

/// Nhớ server nào đã chạy kể từ bản trước và Job nào vừa được bắt đầu.
#[derive(Debug, Default)]
pub struct BackupPlanner {
    ran: HashSet<ServerId>,
    started: HashMap<ServerId, i64>,
}

impl BackupPlanner {
    /// Server cần sao lưu ngay. `now`: giây Unix; `of_day`: giây kể từ nửa đêm theo giờ máy.
    pub fn tick(&mut self, now: i64, of_day: u32, servers: &[BackupCandidate]) -> Vec<ServerId> {
        let mut due = Vec::new();
        for server in servers {
            if server.running {
                self.ran.insert(server.id);
            }
            if self.waiting_for_job(server, now) {
                continue;
            }
            let Some(at) = next_run(server.schedule, server.last, now, of_day) else {
                continue;
            };
            // Chưa có bản nào thì sao lưu luôn; có rồi thì chỉ khi server đã chạy kể từ bản đó.
            if at > now || (server.last.is_some() && !self.ran.contains(&server.id)) {
                continue;
            }
            self.ran.remove(&server.id);
            self.started.insert(server.id, now);
            due.push(server.id);
        }
        due
    }

    /// Job vừa bắt đầu chưa ghi xong bản mới thì không bắt đầu Job thứ hai.
    fn waiting_for_job(&mut self, server: &BackupCandidate, now: i64) -> bool {
        let Some(&started) = self.started.get(&server.id) else {
            return false;
        };
        let done = server.last.is_some_and(|last| last >= started);
        if done || now - started >= RETRY_AFTER_SECS {
            self.started.remove(&server.id);
            return false;
        }
        true
    }
}

impl App {
    /// Chạy lịch sao lưu trong nền. Task giữ `Weak<App>` và dừng khi `App` bị hủy.
    pub fn spawn_backup_schedule(self: &Arc<Self>) {
        if tokio::runtime::Handle::try_current().is_err() {
            tracing::warn!("chưa có runtime tokio, chưa chạy lịch sao lưu");
            return;
        }
        tokio::spawn(run_schedule(Arc::downgrade(self)));
    }

    /// Server có lịch sao lưu, kèm bản tự động gần nhất. Server hỏng `instance.toml` thì bỏ qua.
    pub fn backup_candidates(&self) -> Vec<BackupCandidate> {
        let listed = match self.store.list() {
            Ok(listed) => listed,
            Err(err) => {
                tracing::warn!(error = %err, "không đọc được danh sách server cho lịch sao lưu");
                return Vec::new();
            }
        };
        listed
            .into_iter()
            .filter_map(|item| item.spec.ok())
            .filter(|spec| !spec.backup.schedule.is_off())
            .map(|spec| BackupCandidate {
                id: spec.id,
                schedule: spec.backup.schedule,
                last: self.last_scheduled(spec.id),
                running: self.supervisor(spec.id).status().state != ServerState::Stopped,
            })
            .collect()
    }

    fn last_scheduled(&self, id: ServerId) -> Option<i64> {
        self.list_backups(id)
            .ok()?
            .into_iter()
            .filter(|meta| meta.kind == BackupKind::Scheduled)
            .map(|meta| meta.created_at.unix_timestamp())
            .max()
    }

    /// Bắt đầu một bản tự động. Lỗi chỉ ghi log: lịch của server khác vẫn chạy tiếp.
    pub fn start_scheduled_backup(self: &Arc<Self>, id: ServerId) {
        let req = BackupRequest {
            note: None,
            protected: false,
            kind: BackupKind::Scheduled,
        };
        match self.backup(id, req) {
            Ok(started) => tracing::info!(server = %id, job = started.job_id, "sao lưu theo lịch"),
            Err(err) => {
                tracing::warn!(server = %id, error = %err, "không bắt đầu được sao lưu theo lịch")
            }
        }
    }
}

async fn run_schedule(app: Weak<App>) {
    tokio::time::sleep(FIRST_DELAY).await;
    let mut planner = BackupPlanner::default();
    let mut interval = tokio::time::interval(TICK);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let Some(app) = app.upgrade() else { return };
        let candidates = app.backup_candidates();
        let (now, of_day) = local_now();
        for id in planner.tick(now, of_day, &candidates) {
            app.start_scheduled_backup(id);
        }
    }
}

#[cfg(test)]
#[path = "backup_schedule_tests.rs"]
mod tests;
