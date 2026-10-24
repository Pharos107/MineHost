//! Route `/jobs`, `/catalog` và `/app` (spec P1, mục 5.8).

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use mh_app::App;
use mh_core::LoaderKind;

use crate::dto::{CatalogVersionDto, JobDto};
use crate::error::{ApiError, ErrorBody};

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

#[utoipa::path(get, path = "/api/v1/catalog/paper/versions", tag = "catalog",
    responses(
        (status = 200, description = "Bản release Paper có build STABLE, mới nhất trước", body = [CatalogVersionDto]),
        (status = 502, body = ErrorBody)
    ))]
pub async fn paper_versions(
    State(app): State<Arc<App>>,
) -> Result<Json<Vec<CatalogVersionDto>>, ApiError> {
    let versions = app.catalog_versions(LoaderKind::Paper).await?;
    Ok(Json(versions.into_iter().map(Into::into).collect()))
}

#[utoipa::path(post, path = "/api/v1/app/shutdown", tag = "system",
    responses((status = 202, description = "MineHost đang tắt an toàn mọi server rồi thoát")))]
pub async fn shutdown(State(app): State<Arc<App>>) -> StatusCode {
    tracing::info!("người dùng bấm Thoát");
    app.request_shutdown();
    StatusCode::ACCEPTED
}
