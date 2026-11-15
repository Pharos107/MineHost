//! Route `/jobs`, `/catalog` và `/app` (spec P1 mục 5.8, danh sách Job ở P2).

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use mh_app::App;
use mh_core::{McVersion, ServerId};
use serde::Deserialize;

use crate::dto::{CatalogVersionDto, JobDto, LoaderVersionDto};
use crate::error::{ApiError, ErrorBody};

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct JobsQuery {
    /// ULID của server. Bỏ qua thì lấy mọi Job đang giữ.
    #[serde(default)]
    pub server_id: Option<String>,
}

#[utoipa::path(get, path = "/api/v1/jobs", tag = "jobs", params(JobsQuery),
    responses((status = 200, body = [JobDto]), (status = 400, body = ErrorBody)))]
pub async fn list(
    State(app): State<Arc<App>>,
    Query(query): Query<JobsQuery>,
) -> Result<Json<Vec<JobDto>>, ApiError> {
    let server_id = server_filter(query.server_id)?;
    let jobs = app.jobs().list(server_id);
    Ok(Json(jobs.into_iter().map(JobDto::from).collect()))
}

fn server_filter(raw: Option<String>) -> Result<Option<ServerId>, ApiError> {
    raw.map(|value| ServerId::parse(&value).map_err(|_| ApiError::invalid("server_id")))
        .transpose()
}

#[utoipa::path(get, path = "/api/v1/jobs/{id}", tag = "jobs",
    params(("id" = u64, Path)),
    responses((status = 200, body = JobDto), (status = 404, body = ErrorBody)))]
pub async fn get(
    State(app): State<Arc<App>>,
    Path(id): Path<u64>,
) -> Result<Json<JobDto>, ApiError> {
    let job = app.jobs().get(id).ok_or_else(|| {
        ApiError::from(mh_app::AppError::new(mh_core::ErrorCode::NotFound, "job"))
    })?;
    Ok(Json(job.into()))
}

#[utoipa::path(post, path = "/api/v1/jobs/{id}/cancel", tag = "jobs",
    params(("id" = u64, Path)),
    responses((status = 204, description = "Đã yêu cầu hủy"), (status = 404, body = ErrorBody)))]
pub async fn cancel(
    State(app): State<Arc<App>>,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    app.jobs().cancel(id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/v1/catalog/{loader}/versions", tag = "catalog",
    params(("loader" = String, Path, description = "`paper`, `purpur`, `vanilla`, `fabric`, `quilt`, `forge` hoặc `neoforge`")),
    responses(
        (status = 200, description = "Bản release cài được, mới nhất trước. Paper chỉ tính bản có build STABLE", body = [CatalogVersionDto]),
        (status = 400, description = "Loại server chưa hỗ trợ", body = ErrorBody),
        (status = 502, body = ErrorBody)
    ))]
pub async fn catalog_versions(
    State(app): State<Arc<App>>,
    Path(loader): Path<String>,
) -> Result<Json<Vec<CatalogVersionDto>>, ApiError> {
    let loader = crate::servers::parse_loader(&loader)?;
    let versions = app.catalog_versions(loader).await?;
    Ok(Json(versions.into_iter().map(Into::into).collect()))
}

#[utoipa::path(get, path = "/api/v1/catalog/{loader}/{minecraft}/loader-versions", tag = "catalog",
    params(
        ("loader" = String, Path, description = "`fabric`, `quilt`, `forge` hoặc `neoforge`"),
        ("minecraft" = String, Path)
    ),
    responses(
        (status = 200, description = "Bản loader, bản đề xuất đứng đầu", body = [LoaderVersionDto]),
        (status = 400, description = "Loại server không có bản loader", body = ErrorBody),
        (status = 404, description = "Loader không có bản cho phiên bản này", body = ErrorBody),
        (status = 502, body = ErrorBody)
    ))]
pub async fn loader_versions(
    State(app): State<Arc<App>>,
    Path((loader, minecraft)): Path<(String, String)>,
) -> Result<Json<Vec<LoaderVersionDto>>, ApiError> {
    let loader = crate::servers::parse_loader(&loader)?;
    if !loader.is_mod_loader() {
        return Err(ApiError::invalid("loader"));
    }
    let minecraft = McVersion::parse(&minecraft).map_err(|_| ApiError::invalid("minecraft"))?;
    let versions = app.loader_versions(loader, &minecraft).await?;
    Ok(Json(versions.into_iter().map(Into::into).collect()))
}

#[utoipa::path(post, path = "/api/v1/app/shutdown", tag = "system",
    responses((status = 202, description = "MineHost đang tắt an toàn mọi server rồi thoát")))]
pub async fn shutdown(State(app): State<Arc<App>>) -> StatusCode {
    tracing::info!("người dùng bấm Thoát");
    app.request_shutdown();
    StatusCode::ACCEPTED
}
