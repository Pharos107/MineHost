//! Route tệp (spec P2, mục 5.8). Gốc là thư mục `server/` của một server.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::Json;
use axum::body::Body;
use axum::extract::multipart::{Field, MultipartError};
use axum::extract::{Extension, Multipart, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use mh_app::{App, FileEntry, FileKind};
use mh_core::ErrorCode;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use utoipa::ToSchema;

use crate::dto::TrashedDto;
use crate::error::{ApiError, ApiJson, ErrorBody, IdPath};

type ApiResult<T> = Result<T, ApiError>;

/// Mỗi file upload. Route truyền giá trị này vào `save_upload`; test được hạ xuống.
pub const UPLOAD_MAX_BYTES: u64 = 512 * 1024 * 1024;

/// Axum đếm cả body. Chừa chỗ cho nhiều file và biên multipart, để một file đúng ngưỡng
/// không bị 413 của axum trước khi `save_upload` trả `FILE_TOO_LARGE`.
const UPLOAD_PARTS: u64 = 16;
const UPLOAD_FRAMING: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub(crate) struct UploadLimit(pub u64);

pub(crate) fn upload_body_limit(per_file: u64) -> usize {
    let bytes = per_file
        .saturating_mul(UPLOAD_PARTS)
        .saturating_add(UPLOAD_FRAMING);
    usize::try_from(bytes).unwrap_or(usize::MAX)
}

pub(crate) async fn inject_upload_limit(
    State(max_bytes): State<u64>,
    mut req: Request,
    next: Next,
) -> Response {
    req.extensions_mut().insert(UploadLimit(max_bytes));
    next.run(req).await
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct FileQuery {
    /// Đường dẫn tương đối trong thư mục server. Rỗng là gốc.
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(rename_all = "snake_case")]
pub enum FileKindDto {
    File,
    Dir,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FileEntryDto {
    pub name: String,
    pub kind: FileKindDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// RFC 3339.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TextFileDto {
    pub text: String,
    pub etag: String,
    pub size: u64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TextBody {
    pub text: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EtagDto {
    pub etag: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MkdirBody {
    pub path: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RenameBody {
    pub from: String,
    pub to: String,
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/files", tag = "files",
    params(("id" = String, Path), FileQuery),
    responses((status = 200, body = [FileEntryDto]), (status = 400, body = ErrorBody)))]
pub async fn list(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<FileQuery>,
) -> ApiResult<Json<Vec<FileEntryDto>>> {
    let entries = app.list_dir(id, &query.path).await?;
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        out.push(FileEntryDto::from_entry(entry)?);
    }
    Ok(Json(out))
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/files/content", tag = "files",
    params(("id" = String, Path), FileQuery),
    responses(
        (status = 200, body = TextFileDto),
        (status = 413, body = ErrorBody),
        (status = 415, body = ErrorBody),
    ))]
pub async fn read(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<FileQuery>,
) -> ApiResult<Response> {
    let file = app.read_text(id, &query.path).await?;
    let mut response = Json(TextFileDto {
        text: file.text,
        etag: file.etag.clone(),
        size: file.size,
    })
    .into_response();
    response
        .headers_mut()
        .insert(header::ETAG, quoted_etag(&file.etag)?);
    Ok(response)
}

#[utoipa::path(put, path = "/api/v1/servers/{id}/files/content", tag = "files",
    params(
        ("id" = String, Path),
        FileQuery,
        ("If-Match" = Option<String>, Header, description = "ETag hiện tại. Thiếu khi tạo file mới."),
    ),
    request_body = TextBody,
    responses(
        (status = 200, body = EtagDto),
        (status = 412, description = "If-Match không khớp", body = ErrorBody),
        (status = 413, body = ErrorBody),
        (status = 415, body = ErrorBody),
    ))]
pub async fn write(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<FileQuery>,
    headers: HeaderMap,
    ApiJson(body): ApiJson<TextBody>,
) -> ApiResult<Json<EtagDto>> {
    let etag = app
        .write_text(id, &query.path, &body.text, if_match(&headers).as_deref())
        .await?;
    Ok(Json(EtagDto { etag }))
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/files/download", tag = "files",
    params(("id" = String, Path), FileQuery),
    responses((status = 200, description = "Nội dung file"), (status = 400, body = ErrorBody)))]
pub async fn download(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<FileQuery>,
) -> ApiResult<Response> {
    let (name, file) = app.open_download(id, &query.path).await?;
    download_response(name, file).await
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/files/upload", tag = "files",
    description = "multipart/form-data, một hoặc nhiều phần tên `file`, mỗi file tối đa 512 MiB.",
    params(("id" = String, Path), FileQuery),
    responses((status = 200, body = [String]), (status = 413, body = ErrorBody)))]
pub async fn upload(
    State(app): State<Arc<App>>,
    Extension(UploadLimit(max_bytes)): Extension<UploadLimit>,
    IdPath(id): IdPath,
    Query(query): Query<FileQuery>,
    mut multipart: Multipart,
) -> ApiResult<Json<Vec<String>>> {
    let mut names = Vec::new();
    while let Some(field) = multipart.next_field().await.map_err(from_multipart)? {
        if let Some(name) = save_field(&app, id, &query.path, max_bytes, field).await? {
            names.push(name);
        }
    }
    if names.is_empty() {
        return Err(ApiError::invalid("file"));
    }
    Ok(Json(names))
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/files/mkdir", tag = "files",
    params(("id" = String, Path)), request_body = MkdirBody,
    responses((status = 204, description = "Đã tạo"), (status = 409, body = ErrorBody)))]
pub async fn mkdir(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<MkdirBody>,
) -> ApiResult<StatusCode> {
    app.create_dir(id, &body.path).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/files/rename", tag = "files",
    params(("id" = String, Path)), request_body = RenameBody,
    responses((status = 204, description = "Đã đổi tên"), (status = 409, body = ErrorBody)))]
pub async fn rename_file(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<RenameBody>,
) -> ApiResult<StatusCode> {
    app.rename(id, &body.from, &body.to).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/v1/servers/{id}/files", tag = "files",
    params(("id" = String, Path), FileQuery),
    responses((status = 200, body = TrashedDto), (status = 400, body = ErrorBody)))]
pub async fn delete_file(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    Query(query): Query<FileQuery>,
) -> ApiResult<Json<TrashedDto>> {
    let dest = app.delete(id, &query.path).await?;
    Ok(Json(TrashedDto {
        trash_path: dest.display().to_string(),
    }))
}

impl FileEntryDto {
    fn from_entry(entry: FileEntry) -> Result<Self, ApiError> {
        let modified = match entry.modified {
            Some(time) => Some(time.format(&Rfc3339).map_err(ApiError::internal)?),
            None => None,
        };
        let kind = match entry.kind {
            FileKind::File => FileKindDto::File,
            FileKind::Dir => FileKindDto::Dir,
        };
        Ok(Self {
            name: entry.name,
            kind,
            size: entry.size,
            modified,
        })
    }
}

pub(crate) async fn download_response(name: String, file: tokio::fs::File) -> ApiResult<Response> {
    let length = file.metadata().await.ok().map(|meta| meta.len());
    let mut response = Response::new(Body::from_stream(ReaderStream::new(file)));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, content_type(&name)?);
    headers.insert(header::CONTENT_DISPOSITION, content_disposition(&name)?);
    if let Some(length) = length
        && let Ok(value) = HeaderValue::from_str(&length.to_string())
    {
        headers.insert(header::CONTENT_LENGTH, value);
    }
    Ok(response)
}

async fn save_field(
    app: &App,
    id: mh_core::ServerId,
    dir: &str,
    max_bytes: u64,
    field: Field<'_>,
) -> Result<Option<String>, ApiError> {
    if field.name() != Some("file") {
        drain_field(field).await?;
        return Ok(None);
    }
    let Some(file_name) = field.file_name().filter(|name| !name.is_empty()) else {
        return Err(ApiError::invalid("file"));
    };
    let file_name = file_name.to_owned();
    // Field là stream byte, không phải `AsyncRead`. Không gọi `bytes()` để khỏi gom 512 MiB.
    let reader = tokio_util::io::StreamReader::new(
        field.map(|chunk| chunk.map_err(|err| std::io::Error::other(err.to_string()))),
    );
    app.save_upload(id, dir, &file_name, reader, max_bytes)
        .await?;
    Ok(Some(file_name))
}

pub(crate) async fn drain_field(mut field: Field<'_>) -> Result<(), ApiError> {
    while field.chunk().await.map_err(from_multipart)?.is_some() {}
    Ok(())
}

pub(crate) fn from_multipart(err: MultipartError) -> ApiError {
    if err.status() == StatusCode::PAYLOAD_TOO_LARGE {
        return ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, ErrorCode::FileTooLarge);
    }
    ApiError::invalid("file")
}

static UPLOAD_SEQ: AtomicU64 = AtomicU64::new(0);

pub(crate) struct TempUploadGuard(Option<PathBuf>);

impl TempUploadGuard {
    pub(crate) fn path(&self) -> Option<&std::path::Path> {
        self.0.as_deref()
    }

    pub(crate) fn disarm(mut self) -> Option<PathBuf> {
        self.0.take()
    }
}

impl Drop for TempUploadGuard {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub(crate) async fn stream_to_temp(
    app: &App,
    prefix: &str,
    ext: &str,
    mut field: Field<'_>,
    max_bytes: u64,
) -> ApiResult<TempUploadGuard> {
    let cache_dir = app.dirs().cache();
    tokio::fs::create_dir_all(&cache_dir)
        .await
        .map_err(ApiError::internal)?;
    let seq = UPLOAD_SEQ.fetch_add(1, Ordering::Relaxed);
    let temp_path = cache_dir.join(format!(".{prefix}-{}-{seq}.{ext}.part", std::process::id()));
    let guard = TempUploadGuard(Some(temp_path.clone()));
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(ApiError::internal)?;
    let mut total = 0u64;
    while let Some(chunk) = field.chunk().await.map_err(from_multipart)? {
        total = total.saturating_add(chunk.len() as u64);
        if total > max_bytes {
            return Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                ErrorCode::FileTooLarge,
            ));
        }
        file.write_all(&chunk).await.map_err(ApiError::internal)?;
    }
    file.flush().await.map_err(ApiError::internal)?;
    drop(file);
    Ok(guard)
}

fn if_match(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::IF_MATCH)?.to_str().ok()?.trim();
    if raw.is_empty() {
        return None;
    }
    // `ETag` gửi đi có ngoặc kép (RFC 9110). Dịch vụ so sánh chuỗi hex trần.
    let raw = raw.strip_prefix("W/").unwrap_or(raw).trim();
    Some(raw.trim_matches('"').to_owned())
}

fn quoted_etag(etag: &str) -> Result<HeaderValue, ApiError> {
    HeaderValue::from_str(&format!("\"{etag}\"")).map_err(ApiError::internal)
}

fn content_type(name: &str) -> Result<HeaderValue, ApiError> {
    let mime = mime_guess::from_path(name).first_or_octet_stream();
    HeaderValue::from_str(mime.as_ref()).map_err(ApiError::internal)
}

/// `filename*` theo RFC 5987. Tên Unicode không nhét vào `filename` ASCII được.
fn content_disposition(name: &str) -> Result<HeaderValue, ApiError> {
    let mut value = String::from("attachment; filename*=UTF-8''");
    for byte in name.as_bytes() {
        if is_attr_char(*byte) {
            value.push(char::from(*byte));
        } else {
            value.push_str(&format!("%{byte:02X}"));
        }
    }
    HeaderValue::from_str(&value).map_err(ApiError::internal)
}

fn is_attr_char(byte: u8) -> bool {
    matches!(
        byte,
        b'0'..=b'9'
            | b'a'..=b'z'
            | b'A'..=b'Z'
            | b'!'
            | b'#'
            | b'$'
            | b'&'
            | b'+'
            | b'-'
            | b'.'
            | b'^'
            | b'_'
            | b'`'
            | b'|'
            | b'~'
    )
}
