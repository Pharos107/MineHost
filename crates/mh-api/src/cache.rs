//! `GET`/`DELETE /settings/download-cache`: dung lượng và nút xóa bộ nhớ đệm tải về
//! (spec P3, mục 3.4 và 6.5).

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use mh_app::App;
use serde::Serialize;
use utoipa::ToSchema;

use crate::ApiError;
use crate::error::ErrorBody;

#[derive(Debug, Serialize, ToSchema)]
pub struct DownloadCacheDto {
    /// Tổng byte file mod, plugin, installer, jar server đã tải, lưu theo hash.
    pub size_bytes: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ClearedCacheDto {
    pub freed_bytes: u64,
}

#[utoipa::path(get, path = "/api/v1/settings/download-cache", tag = "settings",
    responses((status = 200, body = DownloadCacheDto)))]
pub async fn get(State(app): State<Arc<App>>) -> Result<Json<DownloadCacheDto>, ApiError> {
    let size_bytes = tokio::task::spawn_blocking(move || app.download_cache_size())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DownloadCacheDto { size_bytes }))
}

#[utoipa::path(delete, path = "/api/v1/settings/download-cache", tag = "settings",
    description = "Xóa file tải về theo hash. Đang có Job tải file thì trả `SERVER_BUSY`.",
    responses(
        (status = 200, body = ClearedCacheDto),
        (status = 409, body = ErrorBody),
    ))]
pub async fn clear(State(app): State<Arc<App>>) -> Result<Json<ClearedCacheDto>, ApiError> {
    let freed_bytes = tokio::task::spawn_blocking(move || app.clear_download_cache())
        .await
        .map_err(ApiError::internal)??;
    Ok(Json(ClearedCacheDto { freed_bytes }))
}
