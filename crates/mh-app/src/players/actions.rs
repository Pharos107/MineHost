//! Thao tác trên một người chơi (bảng ở spec P2, mục 6.7).
//!
//! Server đang chạy: gửi lệnh console, để Minecraft tự sửa file của nó. Server tắt: MineHost
//! sửa thẳng file JSON; thao tác cần người chơi trong game thì trả `SERVER_NOT_RUNNING`.

use std::time::Duration;

use mh_core::{
    ErrorCode, InstanceSpec, PlayerAction, PlayerName, ServerId, ServerState, command, offline_uuid,
};
use mh_platform::SafeRoot;
use mh_store::{BanEntry, OpEntry, PlayerFiles, PlayerRef, WhitelistEntry};
use time::OffsetDateTime;
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;

use crate::launcher::read_properties;
use crate::{App, AppError, SupervisorHandle};

/// Kiểu ngày của file JSON trong Minecraft (`yyyy-MM-dd HH:mm:ss Z` của Java).
pub(crate) const JAVA_DATE: &[BorrowedFormatItem<'static>] = format_description!(
    "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory][offset_minute]"
);

/// Lý do mặc định của Minecraft khi cấm không ghi lý do.
const DEFAULT_BAN_REASON: &str = "Banned by an operator.";
/// Tên hiện ở cột `source` của `banned-players.json` khi MineHost cấm lúc server tắt.
const BAN_SOURCE: &str = "MineHost";
/// Minecraft ghi file sau khi chạy lệnh. Báo giao diện đọc lại lần nữa sau khoảng này.
const FILE_SETTLE: Duration = Duration::from_secs(1);

/// Thao tác app nhận. Dịch chuyển về spawn cần đọc `level.dat` nên không nằm trong [`PlayerAction`].
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerRequest {
    Action(PlayerAction),
    TeleportToSpawn,
}

/// Cách thao tác đã được thực hiện.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionDone {
    /// Đã gửi lệnh console; kết quả hiện trong console.
    Sent,
    /// Server tắt: đã sửa file JSON.
    Saved,
}

impl App {
    pub async fn player_action(
        &self,
        id: ServerId,
        player: &PlayerName,
        request: PlayerRequest,
    ) -> Result<ActionDone, AppError> {
        let handle = self.handle(id)?;
        let done = match handle.status().state {
            ServerState::Running => {
                let action = self.resolve_request(id, request)?;
                self.send_action(id, &handle, player, &action).await?;
                ActionDone::Sent
            }
            ServerState::Stopped | ServerState::Crashed => {
                let PlayerRequest::Action(action) = request else {
                    return Err(not_running());
                };
                self.edit_files(id, player, &action).await?;
                ActionDone::Saved
            }
            _ => {
                return Err(AppError::new(
                    ErrorCode::ServerBusy,
                    "server đang bật hoặc đang tắt",
                ));
            }
        };
        self.online.notify(id);
        if done == ActionDone::Sent {
            let changed = self.online.sender();
            tokio::spawn(async move {
                tokio::time::sleep(FILE_SETTLE).await;
                let _ = changed.send(id);
            });
        }
        Ok(done)
    }

    fn resolve_request(
        &self,
        id: ServerId,
        request: PlayerRequest,
    ) -> Result<PlayerAction, AppError> {
        match request {
            PlayerRequest::Action(action) => Ok(action),
            PlayerRequest::TeleportToSpawn => {
                let (x, y, z) = self.spawn_point(id)?;
                Ok(PlayerAction::TeleportTo { x, y, z })
            }
        }
    }

    async fn send_action(
        &self,
        id: ServerId,
        handle: &SupervisorHandle,
        player: &PlayerName,
        action: &PlayerAction,
    ) -> Result<(), AppError> {
        let freezing = matches!(action, PlayerAction::Freeze | PlayerAction::Unfreeze);
        // Chỉ đóng băng cần thứ tự phiên bản; lệnh khác không đọc thời điểm phát hành.
        let released = if freezing {
            self.release_time(&self.store.load(id)?).await?
        } else {
            OffsetDateTime::UNIX_EPOCH
        };
        for line in command(action, player, released)? {
            handle.send(line).await?;
        }
        let files = self.store.players(id);
        match action {
            PlayerAction::Freeze => {
                files.add_frozen(player)?;
            }
            PlayerAction::Unfreeze => {
                files.remove_frozen(player.as_str())?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn edit_files(
        &self,
        id: ServerId,
        player: &PlayerName,
        action: &PlayerAction,
    ) -> Result<(), AppError> {
        let files = self.store.players(id);
        let by_name = PlayerRef::Name(player.as_str());
        match action {
            PlayerAction::WhitelistAdd => {
                let (uuid, name) = self.resolve_uuid(id, &files, player).await?;
                files.add_whitelist(WhitelistEntry::new(uuid, name))?;
            }
            PlayerAction::WhitelistRemove => {
                files.remove_whitelist(by_name)?;
            }
            PlayerAction::Op => {
                let (uuid, name) = self.resolve_uuid(id, &files, player).await?;
                files.add_op(OpEntry::new(uuid, name, self.op_level(id)?, false))?;
            }
            PlayerAction::Deop => {
                files.remove_op(by_name)?;
            }
            PlayerAction::Ban { reason } => {
                let (uuid, name) = self.resolve_uuid(id, &files, player).await?;
                files.add_ban(ban_entry(uuid, name, reason)?)?;
            }
            PlayerAction::Pardon => {
                files.remove_ban(by_name)?;
            }
            _ => return Err(not_running()),
        }
        Ok(())
    }

    /// UUID Minecraft sẽ dùng cho tên này: offline-mode tính từ tên; online-mode lấy từ
    /// `usercache.json` nếu người đó đã vào, không thì hỏi Mojang. Trả kèm tên đúng hoa thường.
    async fn resolve_uuid(
        &self,
        id: ServerId,
        files: &PlayerFiles,
        player: &PlayerName,
    ) -> Result<(String, PlayerName), AppError> {
        let props = read_properties(&self.store.paths(id).server_dir)?;
        let offline = offline_uuid(player);
        if props.get("online-mode").map(str::trim) == Some("false") {
            return Ok((offline, player.clone()));
        }
        // Bản cache có UUID offline là của lần chạy offline-mode trước, không dùng được.
        let cached = files.read_usercache()?.into_iter().find(|entry| {
            entry.name.as_str().eq_ignore_ascii_case(player.as_str())
                && !entry.uuid.eq_ignore_ascii_case(&offline)
        });
        if let Some(entry) = cached {
            return Ok((entry.uuid, entry.name));
        }
        let profile = self.player_directory.profile(player).await?;
        let name = PlayerName::parse(&profile.name).unwrap_or_else(|_| player.clone());
        Ok((profile.uuid, name))
    }

    /// `op-permission-level` của server, mặc định 4 như Minecraft.
    fn op_level(&self, id: ServerId) -> Result<i32, AppError> {
        let props = read_properties(&self.store.paths(id).server_dir)?;
        Ok(props
            .get("op-permission-level")
            .and_then(|v| v.trim().parse().ok())
            .filter(|level| (1..=4).contains(level))
            .unwrap_or(4))
    }

    /// Điểm spawn của world chính, đọc từ `<level-name>/level.dat`.
    fn spawn_point(&self, id: ServerId) -> Result<(f64, f64, f64), AppError> {
        let dir = self.store.paths(id).server_dir;
        let props = read_properties(&dir)?;
        let level = props
            .get("level-name")
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or("world");
        // `level-name` do người dùng sửa được, nên vẫn đi qua `SafeRoot`.
        let path = SafeRoot::new(&dir)?.join(&format!("{level}/level.dat"))?;
        Ok(mh_store::world_spawn(path.as_path())?)
    }

    async fn release_time(&self, spec: &InstanceSpec) -> Result<OffsetDateTime, AppError> {
        let versions = self.catalog.versions(spec.game.loader).await?;
        versions
            .into_iter()
            .find(|v| v.id == spec.game.minecraft)
            .map(|v| v.release_time)
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::VersionNotFound,
                    format!("không biết ngày phát hành của {}", spec.game.minecraft),
                )
            })
    }
}

fn ban_entry(uuid: String, name: PlayerName, reason: &str) -> Result<BanEntry, AppError> {
    let created = OffsetDateTime::now_utc()
        .format(JAVA_DATE)
        .map_err(|e| AppError::new(ErrorCode::Internal, e.to_string()))?;
    let reason = if reason.trim().is_empty() {
        DEFAULT_BAN_REASON
    } else {
        reason
    };
    Ok(BanEntry::new(
        uuid, name, created, BAN_SOURCE, "forever", reason,
    ))
}

fn not_running() -> AppError {
    AppError::new(ErrorCode::ServerNotRunning, "thao tác cần server đang chạy")
}
