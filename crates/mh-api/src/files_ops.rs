//! Route nhiều mục của trang Tệp: tìm, zip tải xuống, nén, giải nén (spec trang Tệp, mục 3).

use std::sync::Arc;

use axum::Json;
use axum::body::{Body, Bytes};
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use mh_app::{App, FoundEntry, ZipStream};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;

use crate::backups::JobStartedDto;
use crate::error::{ApiError, ApiJson, ErrorBody, IdPath};
use crate::files::{FileKindDto, content_disposition};

type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct SearchQuery {
    /// Thư mục bắt đầu tìm. Rỗng là gốc.
    #[serde(default)]
    pub path: String,
    /// Một phần tên file, không phân biệt hoa thường.
    pub q: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FoundEntryDto {
    /// Đường dẫn tính từ gốc server.
    pub path: String,
    pub kind: FileKindDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// RFC 3339.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResultDto {
    pub items: Vec<FoundEntryDto>,
    /// Còn kết quả chưa trả: tối đa 200 mục, duyệt tối đa 100 000 mục.
    pub truncated: bool,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ArchiveQuery {
    /// Mảng JSON các đường dẫn, ví dụ `["world","server.properties"]`.
    pub paths: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CompressBody {
    pub paths: Vec<String>,
    /// File zip sẽ tạo. Đã có thì `FILE_EXISTS`.
    pub dest: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExtractBody {
    /// File zip.
    pub path: String,
    /// Thư mục đích, tạo nếu chưa có. Rỗng là gốc.
    #[serde(default)]
    pub dest: String,
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/files/search", tag = "files",
    params(("id" = String, Path), SearchQuery),
    responses((status = 200, body = SearchResultDto), (status = 400, body = ErrorBody)))]
pub async fn search(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Json<SearchResultDto>> {
    let result = app.search_files(id, &query.path, &query.q).await?;
    let mut items = Vec::with_capacity(result.items.len());
    for entry in result.items {
        items.push(found_dto(entry)?);
    }
    Ok(Json(SearchResultDto {
        items,
        truncated: result.truncated,
    }))
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/files/archive", tag = "files",
    params(("id" = String, Path), ArchiveQuery),
    responses((status = 200, description = "File zip, stream"), (status = 400, body = ErrorBody)))]
pub async fn archive(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<ArchiveQuery>,
) -> ApiResult<Response> {
    let paths: Vec<String> =
        serde_json::from_str(&query.paths).map_err(|_| ApiError::invalid("paths"))?;
    let (name, rx) = app.zip_download(id, paths).await?;
    let mut response = Response::new(Body::from_stream(zip_body(rx)));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/zip"),
    );
    headers.insert(header::CONTENT_DISPOSITION, content_disposition(&name)?);
    Ok(response)
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/files/compress", tag = "files",
    params(("id" = String, Path)), request_body = CompressBody,
    responses(
        (status = 202, description = "Job nén đã chạy", body = JobStartedDto),
        (status = 409, body = ErrorBody),
    ))]
pub async fn compress(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<CompressBody>,
) -> ApiResult<(StatusCode, Json<JobStartedDto>)> {
    let job_id = app.compress_files(id, body.paths, &body.dest).await?;
    Ok((StatusCode::ACCEPTED, Json(JobStartedDto { job_id })))
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/files/extract", tag = "files",
    params(("id" = String, Path)), request_body = ExtractBody,
    responses(
        (status = 202, description = "Job giải nén đã chạy", body = JobStartedDto),
        (status = 400, body = ErrorBody),
        (status = 409, description = "Trùng file đã có; `params.path` là mục trùng", body = ErrorBody),
    ))]
pub async fn extract(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<ExtractBody>,
) -> ApiResult<(StatusCode, Json<JobStartedDto>)> {
    let job_id = app.extract_files(id, &body.path, &body.dest).await?;
    Ok((StatusCode::ACCEPTED, Json(JobStartedDto { job_id })))
}

fn found_dto(entry: FoundEntry) -> Result<FoundEntryDto, ApiError> {
    let modified = match entry.modified {
        Some(time) => Some(time.format(&Rfc3339).map_err(ApiError::internal)?),
        None => None,
    };
    Ok(FoundEntryDto {
        path: entry.path,
        kind: entry.kind.into(),
        size: entry.size,
        modified,
    })
}

fn zip_body(
    rx: ZipStream,
) -> impl futures_util::Stream<Item = Result<Bytes, std::io::Error>> + Send {
    futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|chunk| (chunk.map(Bytes::from), rx))
    })
}
