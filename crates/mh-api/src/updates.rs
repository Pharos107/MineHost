//! Route kiểm tra và cập nhật mod/plugin (spec P3, mục 5.8). Cả hai đều chạy thành Job.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use mh_app::App;
use mh_app::content::{MAX_UPDATE_ITEMS, UpdateItem};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::backups::JobStartedDto;
use crate::content::parse_source;
use crate::error::{ApiError, ApiJson, ErrorBody, IdPath};

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateItemBody {
    /// `file` trong kết quả kiểm tra, dạng `mods/x.jar`.
    pub file: String,
    pub source: String,
    pub project_id: String,
    /// Bản muốn cài, thường là `latest_version_id`.
    pub version_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateBody {
    pub items: Vec<UpdateItemBody>,
}

/// Dạng `result` của Job `check_updates`. Chỉ để sinh kiểu cho UI; backend ghi bằng `UpdateReport`.
#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateCandidateDto {
    pub file: String,
    /// `modrinth` hoặc `curseforge`.
    pub source: String,
    pub project_id: String,
    pub current_version_id: Option<String>,
    pub latest_version_id: String,
    pub latest_version_name: String,
    /// Mod "Thêm tay" được nhận ra qua hash.
    pub manual: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateReportDto {
    pub updates: Vec<UpdateCandidateDto>,
    pub checked: u32,
    pub unknown: u32,
    pub from_modpack: u32,
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/updates/check", tag = "content",
    description = "Hỏi nguồn mod xem có bản mới. Kết quả nằm trong `result` của Job, dạng `UpdateReportDto`.",
    params(("id" = String, Path)),
    responses(
        (status = 202, body = JobStartedDto),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
    ))]
pub async fn check(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
) -> Result<(StatusCode, Json<JobStartedDto>), ApiError> {
    let job_id = app.check_updates(id)?;
    Ok((StatusCode::ACCEPTED, Json(JobStartedDto { job_id })))
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/updates", tag = "content",
    description = "Tắt server nếu đang chạy, tạo bản `before_update`, rồi cài các bản đã chọn. Lỗi sau \
                   bước sao lưu có `backup_id` trong `params`.",
    params(("id" = String, Path)), request_body = UpdateBody,
    responses(
        (status = 202, body = JobStartedDto),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
        (status = 503, body = ErrorBody),
    ))]
pub async fn update(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<UpdateBody>,
) -> Result<(StatusCode, Json<JobStartedDto>), ApiError> {
    if body.items.is_empty() || body.items.len() > MAX_UPDATE_ITEMS {
        return Err(ApiError::invalid("items"));
    }
    let items = body
        .items
        .iter()
        .map(parse_item)
        .collect::<Result<Vec<_>, _>>()?;
    let job_id = app.update_content(id, items)?;
    Ok((StatusCode::ACCEPTED, Json(JobStartedDto { job_id })))
}

fn parse_item(item: &UpdateItemBody) -> Result<UpdateItem, ApiError> {
    let source = parse_source(&item.source)?;
    let field = |raw: &str, name: &'static str| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            Err(ApiError::invalid(name))
        } else {
            Ok(trimmed.to_owned())
        }
    };
    Ok(UpdateItem {
        file: field(&item.file, "file")?,
        source,
        project_id: field(&item.project_id, "project_id")?,
        version_id: field(&item.version_id, "version_id")?,
    })
}
