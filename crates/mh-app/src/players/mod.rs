//! Người chơi của một server (spec P2, mục 4.4 và 6.7).
//!
//! Danh sách đọc thẳng từ file của Minecraft trong thư mục server, cộng danh sách online từ
//! log. MineHost không giữ bản riêng, trừ `frozen.json` vì Minecraft không có lệnh xem lại.

mod actions;
mod online;

use std::collections::HashMap;
use std::sync::Arc;

use mh_core::{ErrorCode, PlayerName, ServerId, ServerState, offline_uuid};
use mh_net::HttpClient;
use mh_providers::Mojang;
use mh_store::PlayerFiles;
use time::{Date, Month, OffsetDateTime};

use crate::launcher::read_properties;
use crate::sources::BoxFut;
use crate::{App, AppError};

pub use actions::{ActionDone, PlayerRequest};
pub use mh_providers::Profile as PlayerProfile;
pub(crate) use online::OnlinePlayers;

/// Tra tài khoản Minecraft và skin. Bản thật gọi Mojang; test và chế độ e2e thay bằng bản giả.
pub trait PlayerDirectory: Send + Sync + 'static {
    /// `PLAYER_NOT_FOUND` khi không có tài khoản đúng tên này.
    fn profile<'a>(&'a self, name: &'a PlayerName) -> BoxFut<'a, Result<PlayerProfile, AppError>>;
    /// PNG của skin; `NOT_FOUND` khi tài khoản không có skin riêng.
    fn skin<'a>(&'a self, uuid: &'a str) -> BoxFut<'a, Result<Vec<u8>, AppError>>;
}

/// Hồ sơ và skin từ Mojang, skin có cache 24 giờ (spec P2, mục 3.8).
#[derive(Debug)]
pub struct MojangDirectory(Mojang);

impl MojangDirectory {
    pub fn new(http: Arc<HttpClient>) -> Self {
        Self(Mojang::new(http))
    }
}

impl PlayerDirectory for MojangDirectory {
    fn profile<'a>(&'a self, name: &'a PlayerName) -> BoxFut<'a, Result<PlayerProfile, AppError>> {
        Box::pin(async move { Ok(self.0.profile_by_name(name).await?) })
    }

    fn skin<'a>(&'a self, uuid: &'a str) -> BoxFut<'a, Result<Vec<u8>, AppError>> {
        Box::pin(async move { Ok(self.0.skin(uuid).await?) })
    }
}

/// Mặc định của [`App::with_deps`]: không tra được gì, như khi mất mạng.
#[derive(Debug)]
pub struct NoDirectory;

impl PlayerDirectory for NoDirectory {
    fn profile<'a>(&'a self, _: &'a PlayerName) -> BoxFut<'a, Result<PlayerProfile, AppError>> {
        Box::pin(std::future::ready(Err(no_directory())))
    }

    fn skin<'a>(&'a self, _: &'a str) -> BoxFut<'a, Result<Vec<u8>, AppError>> {
        Box::pin(std::future::ready(Err(no_directory())))
    }
}

fn no_directory() -> AppError {
    AppError::new(
        ErrorCode::NetworkUnavailable,
        "không có nguồn tra người chơi",
    )
}

/// Một người chơi, gộp từ mọi file. Có mặt khi xuất hiện ở ít nhất một nguồn.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerView {
    pub name: PlayerName,
    pub uuid: Option<String>,
    pub online: bool,
    /// Từ `usercache.json`: người đã từng vào server này.
    pub last_seen: Option<OffsetDateTime>,
    pub whitelisted: bool,
    pub op: bool,
    /// Lý do cấm; `Some` nghĩa là đang bị cấm.
    pub ban_reason: Option<String>,
    pub frozen: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayersView {
    /// `false` thì chỉ các thao tác sửa file làm được (bảng ở spec P2, mục 6.7).
    pub running: bool,
    pub online_mode: bool,
    pub whitelist_enabled: bool,
    pub max_players: u32,
    /// Đang online trước, rồi người vào gần nhất.
    pub players: Vec<PlayerView>,
}

impl App {
    /// Thay nguồn tra người chơi. `App::new` dùng Mojang; `with_deps` mặc định [`NoDirectory`].
    #[must_use]
    pub fn with_players(mut self, directory: Arc<dyn PlayerDirectory>) -> Self {
        self.player_directory = directory;
        self
    }

    pub fn players(&self, id: ServerId) -> Result<PlayersView, AppError> {
        let state = self.handle(id)?.status().state;
        let props = read_properties(&self.store.paths(id).server_dir)?;
        let online_mode = flag(props.get("online-mode"), true);
        let mut rows = Rows::default();
        self.collect_rows(id, &self.store.players(id), &mut rows)?;
        let mut players = rows.finish(online_mode);
        players.sort_by(|a, b| {
            b.online
                .cmp(&a.online)
                .then(b.last_seen.cmp(&a.last_seen))
                .then_with(|| {
                    a.name
                        .as_str()
                        .to_lowercase()
                        .cmp(&b.name.as_str().to_lowercase())
                })
        });
        Ok(PlayersView {
            running: state == ServerState::Running,
            online_mode,
            whitelist_enabled: flag(props.get("white-list"), false),
            max_players: props
                .get("max-players")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(20),
            players,
        })
    }

    /// Tên người đang online theo log.
    pub fn online_players(&self, id: ServerId) -> Vec<PlayerName> {
        self.online.names(id)
    }

    /// Mỗi lần người chơi của một server đổi (vào, ra, sửa whitelist…) thì nhận id server đó.
    pub fn subscribe_players(&self) -> tokio::sync::broadcast::Receiver<ServerId> {
        self.online.subscribe()
    }

    /// PNG skin qua MineHost, để trình duyệt không gọi thẳng Mojang (spec P2, mục 3.8).
    pub async fn player_skin(&self, uuid: &str) -> Result<Vec<u8>, AppError> {
        self.player_directory.skin(uuid).await
    }

    pub(crate) fn online_count(&self, id: ServerId) -> u32 {
        self.online.count(id)
    }

    fn collect_rows(
        &self,
        id: ServerId,
        files: &PlayerFiles,
        rows: &mut Rows,
    ) -> Result<(), AppError> {
        for name in self.online.names(id) {
            rows.get(&name, None).online = true;
        }
        for entry in files.read_usercache()? {
            rows.get(&entry.name, Some(&entry.uuid)).last_seen = last_seen(&entry.expires_on);
        }
        for entry in files.read_whitelist()? {
            rows.get(&entry.name, Some(&entry.uuid)).whitelisted = true;
        }
        for entry in files.read_ops()? {
            rows.get(&entry.name, Some(&entry.uuid)).op = true;
        }
        for entry in files.read_bans()? {
            rows.get(&entry.name, Some(&entry.uuid)).ban_reason = Some(entry.reason);
        }
        for name in files.read_frozen()? {
            rows.get(&name, None).frozen = true;
        }
        Ok(())
    }
}

/// Gộp theo tên không phân biệt hoa thường; tên hiển thị lấy từ nguồn đầu tiên.
#[derive(Default)]
struct Rows(HashMap<String, PlayerView>);

impl Rows {
    fn get(&mut self, name: &PlayerName, uuid: Option<&str>) -> &mut PlayerView {
        let row = self
            .0
            .entry(name.as_str().to_ascii_lowercase())
            .or_insert_with(|| PlayerView {
                name: name.clone(),
                uuid: None,
                online: false,
                last_seen: None,
                whitelisted: false,
                op: false,
                ban_reason: None,
                frozen: false,
            });
        if row.uuid.is_none() {
            row.uuid = uuid.filter(|u| !u.is_empty()).map(str::to_owned);
        }
        row
    }

    /// Server offline-mode thì UUID tính được từ tên, kể cả người chưa có trong file nào.
    fn finish(self, online_mode: bool) -> Vec<PlayerView> {
        self.0
            .into_values()
            .map(|mut row| {
                if row.uuid.is_none() && !online_mode {
                    row.uuid = Some(offline_uuid(&row.name));
                }
                row
            })
            .collect()
    }
}

/// Giá trị `true`/`false` của `server.properties`; thiếu hoặc lạ thì lấy mặc định của Minecraft.
fn flag(raw: Option<&str>, default: bool) -> bool {
    match raw.map(str::trim) {
        Some("true") => true,
        Some("false") => false,
        _ => default,
    }
}

/// `expiresOn` của `usercache.json` là lần vào cuối cộng một tháng (`Calendar.MONTH` của Java).
fn last_seen(expires_on: &str) -> Option<OffsetDateTime> {
    let expires = OffsetDateTime::parse(expires_on, &crate::players::actions::JAVA_DATE).ok()?;
    let date = expires.date();
    let (year, month) = match date.month() {
        Month::January => (date.year() - 1, Month::December),
        other => (date.year(), other.previous()),
    };
    let day = date.day().min(time::util::days_in_month(month, year));
    let earlier = Date::from_calendar_date(year, month, day).ok()?;
    Some(expires.replace_date(earlier))
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    #[test]
    fn last_seen_is_one_month_before_expiry() {
        assert_eq!(
            last_seen("2026-12-12 10:00:00 +0000"),
            Some(datetime!(2026-11-12 10:00:00 UTC))
        );
        assert_eq!(
            last_seen("2026-01-05 08:30:00 +0700"),
            Some(datetime!(2025-12-05 08:30:00 +7))
        );
        // Hạn 31/3 thì lần vào là 28/2 (Java cắt ngày cuối tháng khi cộng).
        assert_eq!(
            last_seen("2026-03-31 00:00:00 +0000"),
            Some(datetime!(2026-02-28 00:00:00 UTC))
        );
        assert_eq!(last_seen("không phải ngày"), None);
    }

    #[test]
    fn flags_fall_back_to_minecraft_defaults() {
        assert!(flag(None, true));
        assert!(!flag(Some(" false "), true));
        assert!(flag(Some("true"), false));
        assert!(!flag(Some("yes"), false));
    }
}
