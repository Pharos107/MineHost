//! Route bổ sung của trang Tệp: nhân bản, dung lượng thư mục, xem trước, tải từ URL, thùng rác.

use std::sync::Arc;

use axum::Json;
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use mh_app::App;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use tokio_util::io::ReaderStream;
use utoipa::ToSchema;

use crate::backups::JobStartedDto;
use crate::error::{ApiError, ApiJson, ErrorBody, IdPath};
use crate::files::{FileQuery, UPLOAD_MAX_BYTES};

type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CopyBody {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DirSizeDto {
    pub bytes: u64,
    pub files: u64,
    pub dirs: u64,
    pub truncated: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TextPreviewDto {
    pub text: String,
    pub total_bytes: u64,
    pub shown_bytes: u64,
    pub truncated: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NbtPreviewDto {
    pub json: String,
    pub size: u64,
    pub compressed: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FetchBody {
    #[serde(default)]
    pub dir: String,
    pub url: String,
    #[serde(default)]
    pub filename: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TrashBatchDto {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<String>,
    pub items: Vec<String>,
    pub bytes: u64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RestoreTrashBody {
    pub batch: String,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct TrashQuery {
    #[serde(default)]
    pub batch: Option<String>,
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/files/copy", tag = "files",
    params(("id" = String, Path)), request_body = CopyBody,
    responses((status = 204, description = "Đã nhân bản"), (status = 409, body = ErrorBody)))]
pub async fn copy_file(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<CopyBody>,
) -> ApiResult<StatusCode> {
    app.copy_file(id, &body.from, &body.to).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/files/size", tag = "files",
    params(("id" = String, Path), FileQuery),
    responses((status = 200, body = DirSizeDto), (status = 400, body = ErrorBody)))]
pub async fn dir_size(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<FileQuery>,
) -> ApiResult<Json<DirSizeDto>> {
    let size = app.dir_size(id, &query.path).await?;
    Ok(Json(DirSizeDto {
        bytes: size.bytes,
        files: size.files,
        dirs: size.dirs,
        truncated: size.truncated,
    }))
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/files/preview", tag = "files",
    params(("id" = String, Path), FileQuery),
    responses((status = 200, body = TextPreviewDto), (status = 415, body = ErrorBody)))]
pub async fn preview(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<FileQuery>,
) -> ApiResult<Json<TextPreviewDto>> {
    let preview = app.read_preview(id, &query.path).await?;
    Ok(Json(TextPreviewDto {
        text: preview.text,
        total_bytes: preview.total_bytes,
        shown_bytes: preview.shown_bytes,
        truncated: preview.truncated,
    }))
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/files/nbt", tag = "files",
    params(("id" = String, Path), FileQuery),
    responses((status = 200, body = NbtPreviewDto), (status = 400, body = ErrorBody)))]
pub async fn nbt(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<FileQuery>,
) -> ApiResult<Json<NbtPreviewDto>> {
    let nbt = app.read_nbt(id, &query.path).await?;
    Ok(Json(NbtPreviewDto {
        json: nbt.json,
        size: nbt.size,
        compressed: nbt.compressed,
    }))
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/files/raw", tag = "files",
    params(("id" = String, Path), FileQuery),
    responses((status = 200, description = "Nội dung ảnh để xem trước"), (status = 400, body = ErrorBody)))]
pub async fn raw(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<FileQuery>,
) -> ApiResult<Response> {
    let (_name, mime, file) = app.open_image(id, &query.path).await?;
    let length = file.metadata().await.ok().map(|meta| meta.len());
    let mut response = Response::new(Body::from_stream(ReaderStream::new(file)));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if let Some(length) = length
        && let Ok(value) = HeaderValue::from_str(&length.to_string())
    {
        headers.insert(header::CONTENT_LENGTH, value);
    }
    Ok(response)
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/files/fetch", tag = "files",
    params(("id" = String, Path)), request_body = FetchBody,
    responses((status = 202, description = "Job tải từ URL đã chạy", body = JobStartedDto), (status = 409, body = ErrorBody)))]
pub async fn fetch_file(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<FetchBody>,
) -> ApiResult<(StatusCode, Json<JobStartedDto>)> {
    let job_id = app
        .fetch_file(
            id,
            &body.dir,
            &body.url,
            body.filename.as_deref(),
            UPLOAD_MAX_BYTES,
        )
        .await?;
    Ok((StatusCode::ACCEPTED, Json(JobStartedDto { job_id })))
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/files/trash", tag = "files",
    params(("id" = String, Path)),
    responses((status = 200, body = [TrashBatchDto])))]
pub async fn list_trash(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
) -> ApiResult<Json<Vec<TrashBatchDto>>> {
    let batches = app.list_trash(id).await?;
    let mut out = Vec::with_capacity(batches.len());
    for batch in batches {
        let deleted_at = match batch.deleted_at {
            Some(time) => Some(time.format(&Rfc3339).map_err(ApiError::internal)?),
            None => None,
        };
        out.push(TrashBatchDto {
            id: batch.id,
            deleted_at,
            items: batch.items,
            bytes: batch.bytes,
        });
    }
    Ok(Json(out))
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/files/trash/restore", tag = "files",
    params(("id" = String, Path)), request_body = RestoreTrashBody,
    responses((status = 200, body = [String]), (status = 409, body = ErrorBody)))]
pub async fn restore_trash(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<RestoreTrashBody>,
) -> ApiResult<Json<Vec<String>>> {
    let restored = app.restore_trash(id, &body.batch).await?;
    Ok(Json(restored))
}

#[utoipa::path(delete, path = "/api/v1/servers/{id}/files/trash", tag = "files",
    params(("id" = String, Path), TrashQuery),
    responses((status = 204, description = "Đã dọn thùng rác")))]
pub async fn empty_trash(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<TrashQuery>,
) -> ApiResult<StatusCode> {
    app.empty_trash(id, query.batch.as_deref()).await?;
    Ok(StatusCode::NO_CONTENT)
}
