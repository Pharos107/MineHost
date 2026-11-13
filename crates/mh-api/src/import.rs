//! Nhập server có sẵn (spec P2, mục 5.8 và 6.11): xem trước rồi chạy Job chép thư mục.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use mh_app::{App, Detected, ImportRequest};
use mh_core::{LoaderKind, McVersion, ServerName};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::dto::CreatedDto;
use crate::error::{ApiError, ApiJson, ErrorBody};

#[derive(Debug, Deserialize, ToSchema)]
pub struct InspectBody {
    /// Đường dẫn tuyệt đối tới thư mục server trên máy chạy MineHost.
    pub path: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DetectedDto {
    /// `paper`, `purpur`, `vanilla`; không nhận ra thì `null`, người dùng chọn tay.
    pub loader: Option<String>,
    pub minecraft: Option<String>,
    /// Jar sẽ chạy. `null` nghĩa là thư mục không có jar nào, không nhập được.
    pub jar: Option<String>,
    /// Có `mods/`: chưa hỗ trợ ở bản này.
    pub has_mods: bool,
}

impl From<Detected> for DetectedDto {
    fn from(found: Detected) -> Self {
        Self {
            loader: found.loader.map(|l| l.to_string()),
            minecraft: found.minecraft.map(|v| v.to_string()),
            jar: found.jar.map(|j| j.as_str().to_owned()),
            has_mods: found.has_mods,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportBody {
    pub path: String,
    pub name: String,
    /// Chỉ gửi khi người dùng chọn tay; có thì thắng kết quả nhận diện.
    pub loader: Option<String>,
    pub minecraft: Option<String>,
}

#[utoipa::path(post, path = "/api/v1/servers/import/inspect", tag = "servers", request_body = InspectBody,
    responses((status = 200, body = DetectedDto), (status = 400, body = ErrorBody)))]
pub async fn inspect(
    State(app): State<Arc<App>>,
    ApiJson(body): ApiJson<InspectBody>,
) -> Result<Json<DetectedDto>, ApiError> {
    let path = PathBuf::from(body.path);
    // Đọc thư mục và mở jar là I/O chặn.
    let found = tokio::task::spawn_blocking(move || app.inspect_import(&path))
        .await
        .map_err(|_| ApiError::internal("task xem thư mục dừng"))??;
    Ok(Json(found.into()))
}

#[utoipa::path(post, path = "/api/v1/servers/import", tag = "servers", request_body = ImportBody,
    responses(
        (status = 202, description = "Job nhập server đã chạy", body = CreatedDto),
        (status = 400, body = ErrorBody),
        (status = 409, description = "Có `mods/` hoặc cổng bận", body = ErrorBody)
    ))]
pub async fn import(
    State(app): State<Arc<App>>,
    ApiJson(body): ApiJson<ImportBody>,
) -> Result<(StatusCode, Json<CreatedDto>), ApiError> {
    let req = ImportRequest {
        path: PathBuf::from(body.path),
        name: ServerName::parse(&body.name).map_err(|_| ApiError::invalid("name"))?,
        loader: body
            .loader
            .as_deref()
            .map(crate::servers::parse_loader)
            .transpose()?,
        minecraft: body
            .minecraft
            .as_deref()
            .map(McVersion::parse)
            .transpose()
            .map_err(|_| ApiError::invalid("minecraft"))?,
    };
    let created = tokio::task::spawn_blocking(move || app.import(req))
        .await
        .map_err(|_| ApiError::internal("task nhập server dừng"))??;
    let dto = CreatedDto {
        server_id: created.server_id.to_string(),
        job_id: created.job_id,
    };
    Ok((StatusCode::ACCEPTED, Json(dto)))
}
