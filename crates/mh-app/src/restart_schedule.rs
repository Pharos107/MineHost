//! Lịch khởi động lại hằng ngày (spec P2, mục 5.7): báo trước bằng `say` lúc còn 5 phút, 1 phút,
//! 10 giây, rồi `restart`. Server không chạy thì bỏ qua cả lời báo lẫn lần khởi động lại.
//!
//! [`RestartPlanner`] là phần tính toán thuần, nhận giờ từ ngoài vào, nên test được bằng đồng hồ
//! giả. Vòng lặp thật đọc giờ máy mỗi giây.

use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::Duration;

use mh_core::{DailyTime, ServerId, ServerState, seconds_until_next};

use crate::App;

/// Đọc lại lịch từ `instance.toml` mỗi chừng này lượt (một lượt một giây).
const RELOAD_EVERY_TICKS: u32 = 10;
const TICK: Duration = Duration::from_secs(1);

/// Lời báo trước, theo thứ tự gửi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RestartWarning {
    FiveMinutes = 1,
    OneMinute = 2,
    TenSeconds = 3,
}

impl RestartWarning {
    /// Giây còn lại mà từ đó lời báo này tới hạn.
    const fn threshold(self) -> i64 {
        match self {
            Self::FiveMinutes => 300,
            Self::OneMinute => 60,
            Self::TenSeconds => 10,
        }
    }

    /// Câu gửi bằng `say` cho người đang chơi. Người chơi thấy trong game, không qua API.
    pub const fn message(self) -> &'static str {
        match self {
            Self::FiveMinutes => "[MineHost] Server sẽ khởi động lại sau 5 phút.",
            Self::OneMinute => "[MineHost] Server sẽ khởi động lại sau 1 phút.",
            Self::TenSeconds => "[MineHost] Server khởi động lại sau 10 giây.",
        }
    }

    /// Lời báo ứng với số giây còn lại; `None` khi còn hơn 5 phút.
    fn due(left: i64) -> Option<Self> {
        [Self::TenSeconds, Self::OneMinute, Self::FiveMinutes]
            .into_iter()
            .find(|warning| left <= warning.threshold())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestartAction {
    Warn(ServerId, RestartWarning),
    Restart(ServerId),
}

/// Lịch của một server tại một lượt.
#[derive(Debug, Clone)]
pub struct ServerSchedule {
    pub id: ServerId,
    pub times: Vec<DailyTime>,
    /// Server đang chạy (`running`). Chỉ khi đó mới báo và khởi động lại.
    pub running: bool,
}

#[derive(Debug, Clone, Copy)]
struct Pending {
    /// Thời điểm khởi động lại, giây Unix.
    target: i64,
    /// Lời báo cuối cùng đã xử lý.
    last: Option<RestartWarning>,
}

/// Nhớ mốc sắp tới của từng server và lời báo đã gửi, để mỗi lời chỉ gửi một lần.
#[derive(Debug, Default)]
pub struct RestartPlanner {
    pending: HashMap<ServerId, Pending>,
    /// Mốc đã xử lý gần nhất, để hai lượt trong cùng một giây không khởi động lại hai lần.
    done: HashMap<ServerId, i64>,
}

impl RestartPlanner {
    /// `now`: giây Unix. `of_day`: giây kể từ nửa đêm theo giờ máy, cùng thời điểm với `now`.
    pub fn tick(
        &mut self,
        now: i64,
        of_day: u32,
        servers: &[ServerSchedule],
    ) -> Vec<RestartAction> {
        self.pending
            .retain(|id, _| servers.iter().any(|s| s.id == *id && !s.times.is_empty()));
        let mut actions = Vec::new();
        for server in servers {
            if let Some(action) = self.tick_one(now, of_day, server) {
                actions.push(action);
            }
        }
        actions
    }

    fn tick_one(
        &mut self,
        now: i64,
        of_day: u32,
        server: &ServerSchedule,
    ) -> Option<RestartAction> {
        let next = now + i64::from(seconds_until_next(&server.times, of_day)?);
        match self.pending.get(&server.id) {
            // Lượt trước còn trước mốc, lượt này đã qua (đồng hồ nhảy qua đúng giây đó): vẫn làm.
            Some(p) if p.target <= now => return self.fire(server, p.target),
            // Lịch vừa đổi: bỏ mốc cũ, lời báo đã gửi cho mốc cũ không còn đúng.
            Some(p) if p.target != next => {
                self.pending.remove(&server.id);
            }
            _ => {}
        }
        let left = next - now;
        let warning = RestartWarning::due(left);
        let already_done = self.done.get(&server.id).is_some_and(|d| next <= *d);
        // Còn hơn 5 phút thì chưa cần nhớ gì (`due` luôn có giá trị khi còn ≤ 10 giây, kể cả 0).
        if warning.is_none() || already_done {
            return None;
        }
        let pending = self.pending.entry(server.id).or_insert(Pending {
            target: next,
            last: None,
        });
        if pending.target <= now {
            let target = pending.target;
            return self.fire(server, target);
        }
        // Vào giữa chừng (controller vừa bật lúc còn 30 giây) thì chỉ gửi lời báo đang tới hạn.
        let warning = warning.filter(|w| pending.last < Some(*w))?;
        pending.last = Some(warning);
        server
            .running
            .then_some(RestartAction::Warn(server.id, warning))
    }

    fn fire(&mut self, server: &ServerSchedule, target: i64) -> Option<RestartAction> {
        self.pending.remove(&server.id);
        self.done.insert(server.id, target);
        server.running.then_some(RestartAction::Restart(server.id))
    }
}

impl App {
    /// Chạy lịch khởi động lại trong nền. Task giữ `Weak<App>` và dừng khi `App` bị hủy.
    pub fn spawn_restart_schedule(self: &Arc<Self>) {
        if tokio::runtime::Handle::try_current().is_err() {
            tracing::warn!("chưa có runtime tokio, chưa chạy lịch khởi động lại");
            return;
        }
        let app = Arc::downgrade(self);
        tokio::spawn(run_schedule(app));
    }

    /// Lịch của mọi server, kèm server nào đang chạy. Server hỏng `instance.toml` thì bỏ qua.
    pub fn restart_schedules(&self) -> Vec<ServerSchedule> {
        let listed = match self.store.list() {
            Ok(listed) => listed,
            Err(err) => {
                tracing::warn!(error = %err, "không đọc được danh sách server cho lịch khởi động lại");
                return Vec::new();
            }
        };
        listed
            .into_iter()
            .filter_map(|item| item.spec.ok())
            .filter(|spec| !spec.policy.restart_at.is_empty())
            .map(|spec| ServerSchedule {
                id: spec.id,
                running: self.supervisor(spec.id).status().state == ServerState::Running,
                times: spec.policy.restart_at,
            })
            .collect()
    }

    /// Gửi lời báo hoặc khởi động lại. Lỗi chỉ ghi log: lịch của server khác vẫn chạy tiếp.
    pub async fn apply_restart_action(&self, action: &RestartAction) {
        let result = match action {
            RestartAction::Warn(id, warning) => {
                self.send_command(*id, &format!("say {}", warning.message()))
                    .await
            }
            RestartAction::Restart(id) => {
                tracing::info!(server = %id, "khởi động lại theo lịch");
                self.restart(*id).await
            }
        };
        if let Err(err) = result {
            tracing::warn!(?action, error = %err, "lịch khởi động lại không chạy được");
        }
    }
}

async fn run_schedule(app: Weak<App>) {
    let mut planner = RestartPlanner::default();
    let mut schedules = Vec::new();
    let mut interval = tokio::time::interval(TICK);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    for tick in 0u32.. {
        interval.tick().await;
        let Some(app) = app.upgrade() else { return };
        if tick % RELOAD_EVERY_TICKS == 0 {
            schedules = app.restart_schedules();
        } else {
            refresh_running(&app, &mut schedules);
        }
        let (now, of_day) = local_now();
        for action in planner.tick(now, of_day, &schedules) {
            app.apply_restart_action(&action).await;
        }
    }
}

/// Trạng thái chạy đổi nhanh hơn lịch, nên đọc lại mỗi lượt từ supervisor (trong RAM).
fn refresh_running(app: &App, schedules: &mut [ServerSchedule]) {
    for schedule in schedules {
        schedule.running = app.supervisor(schedule.id).status().state == ServerState::Running;
    }
}

/// Giây Unix và giây kể từ nửa đêm theo múi giờ của máy. Không đọc được múi giờ thì dùng UTC.
fn local_now() -> (i64, u32) {
    let now = jiff::Zoned::now();
    let time = now.time();
    let of_day = u32::from(time.hour().unsigned_abs()) * 3600
        + u32::from(time.minute().unsigned_abs()) * 60
        + u32::from(time.second().unsigned_abs());
    (now.timestamp().as_second(), of_day)
}

#[cfg(test)]
#[path = "restart_schedule_tests.rs"]
mod tests;
