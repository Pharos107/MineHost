//! Route sao lưu (spec P2, mục 5.8): danh sách, tạo, sửa, xóa, tải xuống, khôi phục.

use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{FromRequestParts, Path, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::Response;
use mh_app::{App, BackupKind, BackupMeta, BackupRequest};
use mh_core::ServerId;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;

use crate::error::{ApiError, ApiJson, ErrorBody, IdPath};
use crate::files::download_response;

/// Ghi chú hiện trong bảng; dài hơn thì nên viết ở chỗ khác.
const NOTE_MAX_CHARS: usize = 200;

type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupDto {
    pub id: String,
    /// `manual`, `scheduled` hoặc `before_restore`.
    pub kind: String,
    /// RFC 3339.
    pub created_at: String,
    /// Kích thước file `.tar.zst`, byte.
    pub size: u64,
    pub minecraft: String,
    pub loader: String,
    pub note: Option<String>,
    pub protected: bool,
}

impl From<BackupMeta> for BackupDto {
    fn from(meta: BackupMeta) -> Self {
        Self {
            id: meta.id,
            kind: kind_name(meta.kind).to_owned(),
            created_at: meta.created_at.format(&Rfc3339).unwrap_or_default(),
            size: meta.size,
            minecraft: meta.minecraft,
            loader: meta.loader.to_string(),
            note: meta.note,
            protected: meta.protected,
        }
    }
}

const fn kind_name(kind: BackupKind) -> &'static str {
    match kind {
        BackupKind::Manual => "manual",
        BackupKind::Scheduled => "scheduled",
        BackupKind::BeforeRestore => "before_restore",
    }
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct CreateBackupBody {
    pub note: Option<String>,
    /// Bản được bảo vệ không bị tự xóa và không xóa được cho tới khi bỏ bảo vệ.
    pub protected: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchBackupBody {
    /// Chuỗi rỗng là xóa ghi chú.
    pub note: Option<String>,
    pub protected: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RestoreBody {
    /// Phải là `true`: người dùng đã tích "Tôi hiểu" trong hộp thoại.
    pub confirm: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct JobStartedDto {
    pub job_id: u64,
}

/// `{id}` và `{bid}` trong đường dẫn.
pub struct BackupPath(ServerId, String);

impl<S: Send + Sync> FromRequestParts<S> for BackupPath {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(params) = Path::<HashMap<String, String>>::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::invalid("id"))?;
        let id = params
            .get("id")
            .and_then(|raw| ServerId::parse(raw).ok())
            .ok_or_else(|| ApiError::invalid("id"))?;
        let bid = params
            .get("bid")
            .cloned()
            .ok_or_else(|| ApiError::invalid("bid"))?;
        Ok(Self(id, bid))
    }
}

/// Ghi chú: cắt khoảng trắng hai đầu, rỗng là không có, tối đa 200 ký tự, không ký tự điều khiển.
fn clean_note(raw: Option<String>) -> ApiResult<Option<Option<String>>> {
    let Some(raw) = raw else { return Ok(None) };
    let note = raw.trim();
    if note.chars().count() > NOTE_MAX_CHARS || note.chars().any(char::is_control) {
        return Err(ApiError::invalid("note"));
    }
    Ok(Some((!note.is_empty()).then(|| note.to_owned())))
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/backups", tag = "backups",
    params(("id" = String, Path)),
    responses((status = 200, description = "Mới nhất trước", body = [BackupDto]), (status = 404, body = ErrorBody)))]
pub async fn list(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
) -> ApiResult<Json<Vec<BackupDto>>> {
    let list = app.list_backups(id)?;
    Ok(Json(list.into_iter().map(Into::into).collect()))
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/backups", tag = "backups",
    params(("id" = String, Path)), request_body = CreateBackupBody,
    responses((status = 202, description = "Job sao lưu đã chạy", body = JobStartedDto), (status = 400, body = ErrorBody)))]
pub async fn create(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<CreateBackupBody>,
) -> ApiResult<(StatusCode, Json<JobStartedDto>)> {
    let req = BackupRequest {
        note: clean_note(body.note)?.flatten(),
        protected: body.protected.unwrap_or(false),
        kind: BackupKind::Manual,
    };
    let started = app.backup(id, req)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(JobStartedDto {
            job_id: started.job_id,
        }),
    ))
}

#[utoipa::path(patch, path = "/api/v1/servers/{id}/backups/{bid}", tag = "backups",
    params(("id" = String, Path), ("bid" = String, Path)), request_body = PatchBackupBody,
    responses((status = 200, body = BackupDto), (status = 404, body = ErrorBody)))]
pub async fn patch(
    State(app): State<Arc<App>>,
    BackupPath(id, bid): BackupPath,
    ApiJson(body): ApiJson<PatchBackupBody>,
) -> ApiResult<Json<BackupDto>> {
    let note = clean_note(body.note)?;
    let meta = app.update_backup(id, &bid, note, body.protected)?;
    Ok(Json(meta.into()))
}

#[utoipa::path(delete, path = "/api/v1/servers/{id}/backups/{bid}", tag = "backups",
    params(("id" = String, Path), ("bid" = String, Path)),
    responses((status = 204, description = "Đã xóa"), (status = 409, description = "Bản đang được bảo vệ", body = ErrorBody)))]
pub async fn delete(
    State(app): State<Arc<App>>,
    BackupPath(id, bid): BackupPath,
) -> ApiResult<StatusCode> {
    app.delete_backup(id, &bid)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/backups/{bid}/download", tag = "backups",
    params(("id" = String, Path), ("bid" = String, Path)),
    responses((status = 200, description = "File .tar.zst"), (status = 404, body = ErrorBody)))]
pub async fn download(
    State(app): State<Arc<App>>,
    BackupPath(id, bid): BackupPath,
) -> ApiResult<Response> {
    let (name, file) = app.open_backup(id, &bid).await?;
    download_response(name, file).await
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/backups/{bid}/restore", tag = "backups",
    params(("id" = String, Path), ("bid" = String, Path)), request_body = RestoreBody,
    responses(
        (status = 202, description = "Job khôi phục đã chạy", body = JobStartedDto),
        (status = 400, description = "Thiếu xác nhận", body = ErrorBody),
        (status = 404, body = ErrorBody)
    ))]
pub async fn restore(
    State(app): State<Arc<App>>,
    BackupPath(id, bid): BackupPath,
    ApiJson(body): ApiJson<RestoreBody>,
) -> ApiResult<(StatusCode, Json<JobStartedDto>)> {
    if !body.confirm {
        return Err(ApiError::invalid("confirm"));
    }
    let started = app.restore(id, &bid)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(JobStartedDto {
            job_id: started.job_id,
        }),
    ))
}
