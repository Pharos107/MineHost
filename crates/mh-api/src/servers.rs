//! Route `/servers` (spec P1, mục 5.8).

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use mh_app::{App, CreateRequest, SettingsPatch};
use mh_core::{
    BackupSchedule, DailyTime, GamePort, LoaderKind, McVersion, MemoryLimits, ServerName,
};
use serde::Deserialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::dto::{
    CommandBody, ConsoleDto, CreateServerBody, CreatedDto, EulaBody, MemoryDto, PatchServerBody,
    PropertiesDto, PropertyDto, ServerDto, ServerMetricsDto, TrashedDto,
};
use crate::error::{ApiError, ApiJson, ErrorBody, IdPath};

type ApiResult<T> = Result<T, ApiError>;

pub(crate) fn parse_loader(s: &str) -> Result<LoaderKind, ApiError> {
    LoaderKind::parse(s).map_err(|_| ApiError::invalid("loader"))
}

#[utoipa::path(get, path = "/api/v1/servers", tag = "servers",
    responses((status = 200, body = [ServerDto])))]
pub async fn list(State(app): State<Arc<App>>) -> ApiResult<Json<Vec<ServerDto>>> {
    Ok(Json(app.list()?.into_iter().map(ServerDto::from).collect()))
}

#[utoipa::path(post, path = "/api/v1/servers", tag = "servers", request_body = CreateServerBody,
    responses(
        (status = 202, description = "Job cài server đã chạy", body = CreatedDto),
        (status = 400, body = ErrorBody), (status = 409, body = ErrorBody)
    ))]
pub async fn create(
    State(app): State<Arc<App>>,
    ApiJson(body): ApiJson<CreateServerBody>,
) -> ApiResult<(StatusCode, Json<CreatedDto>)> {
    let req = CreateRequest {
        name: ServerName::parse(&body.name).map_err(|_| ApiError::invalid("name"))?,
        loader: parse_loader(&body.loader)?,
        minecraft: McVersion::parse(&body.minecraft).map_err(|_| ApiError::invalid("minecraft"))?,
        loader_version: None,
        memory_mb: body.memory_mb.map(memory).transpose()?,
        port: body.port.map(port).transpose()?,
    };
    let created = app.create(req)?;
    let dto = CreatedDto {
        server_id: created.server_id.to_string(),
        job_id: created.job_id,
    };
    Ok((StatusCode::ACCEPTED, Json(dto)))
}

#[utoipa::path(get, path = "/api/v1/servers/{id}", tag = "servers",
    params(("id" = String, Path)),
    responses((status = 200, body = ServerDto), (status = 404, body = ErrorBody)))]
pub async fn get(State(app): State<Arc<App>>, IdPath(id): IdPath) -> ApiResult<Json<ServerDto>> {
    Ok(Json(app.get(id)?.into()))
}

#[utoipa::path(patch, path = "/api/v1/servers/{id}", tag = "servers",
    params(("id" = String, Path)), request_body = PatchServerBody,
    responses((status = 200, body = ServerDto), (status = 409, body = ErrorBody)))]
pub async fn patch(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<PatchServerBody>,
) -> ApiResult<Json<ServerDto>> {
    let name = body.name.as_deref().map(ServerName::parse).transpose();
    let patch = SettingsPatch {
        name: name.map_err(|_| ApiError::invalid("name"))?,
        memory_mb: body.memory_mb.map(memory).transpose()?,
        port: body.port.map(port).transpose()?,
        autostart: body.autostart,
        restart_on_crash: body.restart_on_crash,
        restart_at: body.restart_at.as_deref().map(restart_times).transpose()?,
        backup_schedule: body
            .backup
            .as_ref()
            .and_then(|b| b.schedule.as_deref())
            .map(BackupSchedule::parse)
            .transpose()
            .map_err(|_| ApiError::invalid("backup.schedule"))?,
        backup_keep: body.backup.as_ref().and_then(|b| b.keep),
    };
    app.update_settings(id, patch)?;
    Ok(Json(app.get(id)?.into()))
}

fn restart_times(raw: &[String]) -> ApiResult<Vec<DailyTime>> {
    raw.iter()
        .map(|time| DailyTime::parse(time).map_err(|_| ApiError::invalid("restart_at")))
        .collect()
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct DeleteQuery {
    /// Tên server người dùng gõ lại để xác nhận.
    pub confirm_name: String,
}

#[utoipa::path(delete, path = "/api/v1/servers/{id}", tag = "servers",
    params(("id" = String, Path), DeleteQuery),
    responses((status = 200, body = TrashedDto), (status = 409, body = ErrorBody)))]
pub async fn trash(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<DeleteQuery>,
) -> ApiResult<Json<TrashedDto>> {
    let dest = app.trash(id, &query.confirm_name)?;
    Ok(Json(TrashedDto {
        trash_path: dest.display().to_string(),
    }))
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/eula", tag = "servers",
    params(("id" = String, Path)), request_body = EulaBody,
    responses((status = 200, body = ServerDto), (status = 400, body = ErrorBody)))]
pub async fn eula(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<EulaBody>,
) -> ApiResult<Json<ServerDto>> {
    if !body.accept {
        return Err(ApiError::invalid("accept"));
    }
    app.accept_eula(id)?;
    Ok(Json(app.get(id)?.into()))
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/actions/{action}", tag = "servers",
    params(("id" = String, Path), ("action" = String, Path, description = "start, stop, restart, kill")),
    responses((status = 204, description = "Đã nhận lệnh"), (status = 409, body = ErrorBody)))]
pub async fn action(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Path((_, action)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    match action.as_str() {
        "start" => app.start(id).await?,
        "stop" => app.stop(id).await?,
        "restart" => app.restart(id).await?,
        "kill" => app.kill(id).await?,
        _ => return Err(ApiError::invalid("action")),
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/console", tag = "console",
    params(("id" = String, Path)), request_body = CommandBody,
    responses((status = 204, description = "Đã gửi lệnh"), (status = 409, body = ErrorBody)))]
pub async fn send_command(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<CommandBody>,
) -> ApiResult<StatusCode> {
    let command = body.command.trim();
    if command.is_empty() {
        return Err(ApiError::invalid("command"));
    }
    app.send_command(id, command).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ConsoleQuery {
    /// Chỉ lấy các dòng có `seq` lớn hơn giá trị này.
    pub after_seq: Option<u64>,
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/console", tag = "console",
    params(("id" = String, Path), ConsoleQuery),
    responses((status = 200, body = ConsoleDto), (status = 404, body = ErrorBody)))]
pub async fn console(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<ConsoleQuery>,
) -> ApiResult<Json<ConsoleDto>> {
    let handle = app.handle(id)?;
    let lines = handle.console().after(query.after_seq);
    Ok(Json(ConsoleDto {
        lines: lines.into_iter().map(Into::into).collect(),
    }))
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/properties", tag = "servers",
    params(("id" = String, Path)),
    responses((status = 200, body = PropertiesDto), (status = 404, body = ErrorBody)))]
pub async fn get_properties(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
) -> ApiResult<Json<PropertiesDto>> {
    let properties = app
        .read_properties(id)?
        .into_iter()
        .map(|(key, value)| PropertyDto { key, value })
        .collect();
    Ok(Json(PropertiesDto { properties }))
}

#[utoipa::path(put, path = "/api/v1/servers/{id}/properties", tag = "servers",
    params(("id" = String, Path)), request_body = PropertiesDto,
    responses((status = 204, description = "Đã lưu"), (status = 400, body = ErrorBody)))]
pub async fn put_properties(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<PropertiesDto>,
) -> ApiResult<StatusCode> {
    let entries: Vec<(String, String)> = body
        .properties
        .into_iter()
        .map(|p| (p.key, p.value))
        .collect();
    app.write_properties(id, &entries)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct MetricsQuery {
    /// RFC 3339. Chỉ lấy mẫu sau thời điểm này. Dấu `+` trong offset phải viết `%2B`.
    pub since: Option<String>,
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/metrics", tag = "servers",
    params(("id" = String, Path), MetricsQuery),
    responses(
        (status = 200, body = ServerMetricsDto),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
    ))]
pub async fn metrics(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<MetricsQuery>,
) -> ApiResult<Json<ServerMetricsDto>> {
    let since = query.since.as_deref().map(parse_since).transpose()?;
    Ok(Json(app.metrics(id, since)?.into()))
}

fn parse_since(text: &str) -> Result<OffsetDateTime, ApiError> {
    OffsetDateTime::parse(text, &Rfc3339).map_err(|_| ApiError::invalid("since"))
}

fn memory(m: MemoryDto) -> ApiResult<MemoryLimits> {
    MemoryLimits::new(m.min, m.max).map_err(|_| ApiError::invalid("memory_mb"))
}

fn port(p: u16) -> ApiResult<GamePort> {
    GamePort::new(p).map_err(|_| ApiError::invalid("port"))
}
