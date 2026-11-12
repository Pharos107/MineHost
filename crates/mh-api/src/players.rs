//! Route người chơi (spec P2, mục 5.8 và 6.7): danh sách, thao tác, skin.

use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{FromRequestParts, Path, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use mh_app::{ActionDone, App, PlayerRequest, PlayerView, PlayersView};
use mh_core::{PlayerAction, PlayerName, ServerId};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;

use crate::error::{ApiError, ApiJson, ErrorBody, IdPath};

/// Lý do đuổi, cấm: một dòng ngắn hiện cho người chơi.
const REASON_MAX_CHARS: usize = 200;
/// Tin nhắn và tiêu đề: đủ cho một câu, không thành đoạn văn trong chat.
const TEXT_MAX_CHARS: usize = 256;

type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Serialize, ToSchema)]
pub struct PlayerDto {
    pub name: String,
    /// Có gạch ngang. `None` khi server online-mode và người này chưa vào lần nào.
    pub uuid: Option<String>,
    pub online: bool,
    /// RFC 3339. Có khi người này đã từng vào server.
    pub last_seen: Option<String>,
    pub whitelisted: bool,
    pub op: bool,
    pub banned: bool,
    pub ban_reason: Option<String>,
    pub frozen: bool,
}

impl From<PlayerView> for PlayerDto {
    fn from(view: PlayerView) -> Self {
        Self {
            name: view.name.to_string(),
            uuid: view.uuid,
            online: view.online,
            last_seen: view.last_seen.and_then(|at| at.format(&Rfc3339).ok()),
            whitelisted: view.whitelisted,
            op: view.op,
            banned: view.ban_reason.is_some(),
            ban_reason: view.ban_reason,
            frozen: view.frozen,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PlayersDto {
    /// `false` thì chỉ whitelist, op, cấm, bỏ cấm làm được (MineHost sửa file).
    pub running: bool,
    pub online_mode: bool,
    pub whitelist_enabled: bool,
    pub max_players: u32,
    /// Đang online trước, rồi người vào gần nhất.
    pub players: Vec<PlayerDto>,
}

impl From<PlayersView> for PlayersDto {
    fn from(view: PlayersView) -> Self {
        Self {
            running: view.running,
            online_mode: view.online_mode,
            whitelist_enabled: view.whitelist_enabled,
            max_players: view.max_players,
            players: view.players.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlayerActionKind {
    WhitelistAdd,
    WhitelistRemove,
    Op,
    Deop,
    Kick,
    Ban,
    Pardon,
    Freeze,
    Unfreeze,
    TeleportToPlayer,
    TeleportToSpawn,
    Message,
    Title,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PlayerActionBody {
    pub action: PlayerActionKind,
    /// `kick`, `ban`: tối đa 200 ký tự, có thể bỏ trống.
    pub reason: Option<String>,
    /// `teleport_to_player`: tên người chơi đích.
    pub target: Option<String>,
    /// `message`, `title`: tối đa 256 ký tự.
    pub text: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PlayerActionDto {
    /// `sent`: đã gửi lệnh console. `saved`: server tắt, MineHost đã sửa file.
    pub done: String,
}

/// `{id}` và `{name}` trong đường dẫn.
pub struct PlayerPath(ServerId, PlayerName);

impl<S: Send + Sync> FromRequestParts<S> for PlayerPath {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(params) = Path::<HashMap<String, String>>::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::invalid("id"))?;
        let id = params
            .get("id")
            .and_then(|raw| ServerId::parse(raw).ok())
            .ok_or_else(|| ApiError::invalid("id"))?;
        let name = params
            .get("name")
            .and_then(|raw| PlayerName::parse(raw).ok())
            .ok_or_else(|| ApiError::invalid("name"))?;
        Ok(Self(id, name))
    }
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/players", tag = "players",
    params(("id" = String, Path)),
    responses((status = 200, body = PlayersDto), (status = 404, body = ErrorBody)))]
pub async fn list(State(app): State<Arc<App>>, IdPath(id): IdPath) -> ApiResult<Json<PlayersDto>> {
    Ok(Json(app.players(id)?.into()))
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/players/{name}/actions", tag = "players",
    params(("id" = String, Path), ("name" = String, Path)), request_body = PlayerActionBody,
    responses(
        (status = 200, body = PlayerActionDto),
        (status = 400, body = ErrorBody),
        (status = 404, description = "Không có server, hoặc không có tài khoản Minecraft tên này", body = ErrorBody),
        (status = 409, description = "Thao tác cần server đang chạy, hoặc bản Minecraft không hỗ trợ", body = ErrorBody)
    ))]
pub async fn action(
    State(app): State<Arc<App>>,
    PlayerPath(id, name): PlayerPath,
    ApiJson(body): ApiJson<PlayerActionBody>,
) -> ApiResult<Json<PlayerActionDto>> {
    let done = app.player_action(id, &name, request_of(body)?).await?;
    let done = match done {
        ActionDone::Sent => "sent",
        ActionDone::Saved => "saved",
    };
    Ok(Json(PlayerActionDto {
        done: done.to_owned(),
    }))
}

#[utoipa::path(get, path = "/api/v1/players/{uuid}/skin", tag = "players",
    params(("uuid" = String, Path)),
    responses(
        (status = 200, description = "Ảnh PNG", content_type = "image/png"),
        (status = 404, description = "Tài khoản không có skin riêng", body = ErrorBody),
        (status = 502, description = "Không tải được và chưa có trong cache", body = ErrorBody)
    ))]
pub async fn skin(State(app): State<Arc<App>>, Path(uuid): Path<String>) -> ApiResult<Response> {
    let png = app.player_skin(&uuid).await?;
    // Cache phía MineHost sống 24 giờ; trình duyệt giữ 1 giờ là đủ cho một phiên xem trang.
    Ok((
        [
            (CONTENT_TYPE, "image/png"),
            (CACHE_CONTROL, "private, max-age=3600"),
        ],
        png,
    )
        .into_response())
}

fn request_of(body: PlayerActionBody) -> ApiResult<PlayerRequest> {
    use PlayerActionKind as Kind;
    let action = match body.action {
        Kind::WhitelistAdd => PlayerAction::WhitelistAdd,
        Kind::WhitelistRemove => PlayerAction::WhitelistRemove,
        Kind::Op => PlayerAction::Op,
        Kind::Deop => PlayerAction::Deop,
        Kind::Kick => PlayerAction::Kick {
            reason: clean_reason(body.reason)?,
        },
        Kind::Ban => PlayerAction::Ban {
            reason: clean_reason(body.reason)?,
        },
        Kind::Pardon => PlayerAction::Pardon,
        Kind::Freeze => PlayerAction::Freeze,
        Kind::Unfreeze => PlayerAction::Unfreeze,
        Kind::TeleportToPlayer => {
            let target = body
                .target
                .as_deref()
                .and_then(|raw| PlayerName::parse(raw.trim()).ok())
                .ok_or_else(|| ApiError::invalid("target"))?;
            PlayerAction::TeleportToPlayer { target }
        }
        Kind::TeleportToSpawn => return Ok(PlayerRequest::TeleportToSpawn),
        Kind::Message => PlayerAction::Message {
            text: clean_text(body.text)?,
        },
        Kind::Title => PlayerAction::Title {
            text: clean_text(body.text)?,
        },
    };
    Ok(PlayerRequest::Action(action))
}

fn clean_reason(raw: Option<String>) -> ApiResult<String> {
    let reason = raw.unwrap_or_default().trim().to_owned();
    if reason.chars().count() > REASON_MAX_CHARS || reason.chars().any(char::is_control) {
        return Err(ApiError::invalid("reason"));
    }
    Ok(reason)
}

fn clean_text(raw: Option<String>) -> ApiResult<String> {
    let text = raw.unwrap_or_default().trim().to_owned();
    let count = text.chars().count();
    if count == 0 || count > TEXT_MAX_CHARS || text.chars().any(char::is_control) {
        return Err(ApiError::invalid("text"));
    }
    Ok(text)
}
