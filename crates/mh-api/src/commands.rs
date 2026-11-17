//! Lệnh của server để ô lệnh console gợi ý (spec `lenh-cua-server.md`).

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use mh_app::{App, CommandList, RegistryList};
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;

use crate::error::{ApiError, ErrorBody, IdPath};

type ApiResult<T> = Result<T, ApiError>;

/// Một lệnh đọc từ `help` của server.
#[derive(Debug, Serialize, ToSchema)]
pub struct ServerCommandDto {
    /// Không có dấu `/`, ví dụ `tp` hoặc `ftbquests`.
    pub name: String,
    /// Cách gõ, không có dấu `/`, tối đa 300 ký tự, ví dụ `tp <targets> <location>`.
    pub usage: Option<String>,
    /// Chỉ có ở Paper, Purpur (trang trợ giúp của Bukkit).
    pub description: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ServerCommandsDto {
    /// RFC 3339. `null` khi chưa quét lần nào.
    pub scanned_at: Option<String>,
    pub commands: Vec<ServerCommandDto>,
}

impl From<CommandList> for ServerCommandsDto {
    fn from(list: CommandList) -> Self {
        Self {
            scanned_at: list.scanned_at.and_then(|at| at.format(&Rfc3339).ok()),
            commands: list
                .commands
                .into_iter()
                .map(|c| ServerCommandDto {
                    name: c.name,
                    usage: c.usage,
                    description: c.description,
                })
                .collect(),
        }
    }
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/commands", tag = "console",
    params(("id" = String, Path)),
    responses((status = 200, body = ServerCommandsDto), (status = 404, body = ErrorBody)))]
pub async fn list(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
) -> ApiResult<Json<ServerCommandsDto>> {
    Ok(Json(app.server_commands(id)?.into()))
}

/// Gửi `help` rồi đọc kết quả, mất vài giây. Server phải đang chạy.
#[utoipa::path(post, path = "/api/v1/servers/{id}/commands/scan", tag = "console",
    params(("id" = String, Path)),
    responses((status = 200, body = ServerCommandsDto), (status = 409, body = ErrorBody)))]
pub async fn scan(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
) -> ApiResult<Json<ServerCommandsDto>> {
    Ok(Json(app.scan_commands(id).await?.into()))
}

/// Một mục trong registry của mod (`minecraft:diamond`, `create:brass_ingot`…).
#[derive(Debug, Serialize, ToSchema)]
pub struct RegistryEntryDto {
    pub id: String,
    pub label: Option<String>,
}

/// Tên vật phẩm, hiệu ứng, sinh vật, phù phép trích từ `mods/*.jar` của server.
#[derive(Debug, Serialize, ToSchema)]
pub struct ServerRegistryDto {
    pub scanned_at: Option<String>,
    pub items: Vec<RegistryEntryDto>,
    pub effects: Vec<RegistryEntryDto>,
    pub entities: Vec<RegistryEntryDto>,
    pub enchantments: Vec<RegistryEntryDto>,
}

impl From<RegistryList> for ServerRegistryDto {
    fn from(reg: RegistryList) -> Self {
        let map = |list: Vec<mh_core::commands::RegistryEntry>| {
            list.into_iter()
                .map(|e| RegistryEntryDto {
                    id: e.id,
                    label: e.label,
                })
                .collect()
        };
        Self {
            scanned_at: reg.scanned_at.and_then(|at| at.format(&Rfc3339).ok()),
            items: map(reg.items),
            effects: map(reg.effects),
            entities: map(reg.entities),
            enchantments: map(reg.enchantments),
        }
    }
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/commands/registry", tag = "console",
    params(("id" = String, Path)),
    responses((status = 200, body = ServerRegistryDto), (status = 404, body = ErrorBody)))]
pub async fn registry(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
) -> ApiResult<Json<ServerRegistryDto>> {
    Ok(Json(app.server_registry(id).await?.into()))
}
