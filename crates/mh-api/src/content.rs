//! Route tìm kiếm, xem và quản lý mod/plugin (spec P3, mục 5.8).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::Json;
use axum::extract::multipart::Field;
use axum::extract::{Extension, FromRequestParts, Multipart, Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use mh_app::App;
use mh_app::content::{CONTENT_PAGE_SIZE, ProjectType, SearchResult};
use mh_core::{ErrorCode, ServerId};
use tokio::io::AsyncWriteExt;

use crate::backups::JobStartedDto;
pub(crate) use crate::content_dto::*;
use crate::error::{ApiError, ApiJson, ErrorBody, IdPath};
use crate::files::{self, UploadLimit};

type ApiResult<T> = Result<T, ApiError>;

static UPLOAD_SEQ: AtomicU64 = AtomicU64::new(0);

/// `{id}` và `{file}` trong đường dẫn.
pub struct ContentFilePath(ServerId, String);

impl<S: Send + Sync> FromRequestParts<S> for ContentFilePath {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AxumPath(params) =
            AxumPath::<HashMap<String, String>>::from_request_parts(parts, state)
                .await
                .map_err(|_| ApiError::invalid("id"))?;
        let id = params
            .get("id")
            .and_then(|raw| ServerId::parse(raw).ok())
            .ok_or_else(|| ApiError::invalid("id"))?;
        let file = params
            .get("file")
            .filter(|raw| !raw.is_empty())
            .cloned()
            .ok_or_else(|| ApiError::invalid("file"))?;
        Ok(Self(id, file))
    }
}

#[utoipa::path(get, path = "/api/v1/content/search", tag = "content", params(SearchQuery),
    responses(
        (status = 200, body = SearchResultDto),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
        (status = 502, body = ErrorBody),
        (status = 503, body = ErrorBody),
    ))]
pub async fn search(
    State(app): State<Arc<App>>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Json<SearchResultDto>> {
    check_source(query.source.as_deref().unwrap_or(SOURCE_MODRINTH))?;
    let kind = query.kind.as_deref().map(parse_kind).transpose()?;
    let server_id = query
        .server_id
        .as_deref()
        .map(|raw| ServerId::parse(raw).map_err(|_| ApiError::invalid("server_id")))
        .transpose()?;
    let page = query.page.unwrap_or(1);
    if page == 0 {
        return Err(ApiError::invalid("page"));
    }
    let result: SearchResult = app
        .search_content(query.q.as_deref().unwrap_or(""), kind, server_id, page)
        .await?;
    Ok(Json(SearchResultDto {
        hits: result.hits.into_iter().map(Into::into).collect(),
        page,
        page_size: CONTENT_PAGE_SIZE,
        total: result.total_hits,
    }))
}

#[utoipa::path(get, path = "/api/v1/content/{source}/projects/{pid}", tag = "content",
    params(("source" = String, Path), ("pid" = String, Path)),
    responses(
        (status = 200, body = ProjectDto),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 502, body = ErrorBody),
        (status = 503, body = ErrorBody),
    ))]
pub async fn project(
    State(app): State<Arc<App>>,
    AxumPath((source, pid)): AxumPath<(String, String)>,
) -> ApiResult<Json<ProjectDto>> {
    check_source(&source)?;
    let pid = pid.trim();
    if pid.is_empty() {
        return Err(ApiError::invalid("pid"));
    }
    Ok(Json(app.content_project(pid).await?.into()))
}

#[utoipa::path(get, path = "/api/v1/content/{source}/projects/{pid}/versions", tag = "content",
    params(("source" = String, Path), ("pid" = String, Path), VersionsQuery),
    responses(
        (status = 200, body = [ContentVersionDto]),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
        (status = 502, body = ErrorBody),
        (status = 503, body = ErrorBody),
    ))]
pub async fn versions(
    State(app): State<Arc<App>>,
    AxumPath((source, pid)): AxumPath<(String, String)>,
    Query(query): Query<VersionsQuery>,
) -> ApiResult<Json<Vec<ContentVersionDto>>> {
    check_source(&source)?;
    let pid = pid.trim();
    if pid.is_empty() {
        return Err(ApiError::invalid("pid"));
    }
    let raw_id = query
        .server_id
        .as_deref()
        .ok_or_else(|| ApiError::invalid("server_id"))?;
    let server_id = ServerId::parse(raw_id).map_err(|_| ApiError::invalid("server_id"))?;
    let list = app.content_versions(pid, server_id).await?;
    Ok(Json(list.into_iter().map(Into::into).collect()))
}

#[utoipa::path(get, path = "/api/v1/servers/{id}/content", tag = "content",
    params(("id" = String, Path)),
    responses(
        (status = 200, body = ServerContentDto),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
    ))]
pub async fn list(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
) -> ApiResult<Json<ServerContentDto>> {
    Ok(Json(app.content(id).await?.into()))
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/content/plan", tag = "content",
    params(("id" = String, Path)), request_body = ContentInstallBody,
    responses(
        (status = 200, body = ContentPlanDto),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
        (status = 502, body = ErrorBody),
        (status = 503, body = ErrorBody),
    ))]
pub async fn plan(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<ContentInstallBody>,
) -> ApiResult<Json<ContentPlanDto>> {
    let (project_id, version_id) = validate_install_body(&body)?;
    let plan = app
        .plan_content_install(id, project_id, version_id.as_deref())
        .await?;
    Ok(Json(plan.into()))
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/content", tag = "content",
    params(("id" = String, Path)), request_body = ContentInstallBody,
    responses(
        (status = 202, description = "Job cài mod/plugin đã chạy", body = JobStartedDto),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
        (status = 503, body = ErrorBody),
    ))]
pub async fn install(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
    ApiJson(body): ApiJson<ContentInstallBody>,
) -> ApiResult<(StatusCode, Json<JobStartedDto>)> {
    let (project_id, version_id) = validate_install_body(&body)?;
    let job_id = app.install_content(id, project_id, version_id)?;
    Ok((StatusCode::ACCEPTED, Json(JobStartedDto { job_id })))
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/content/upload", tag = "content",
    description = "multipart/form-data, một file .jar tối đa 512 MiB.",
    params(("id" = String, Path)),
    responses(
        (status = 200, body = InstalledContentDto),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
        (status = 413, body = ErrorBody),
    ))]
pub async fn upload(
    State(app): State<Arc<App>>,
    Extension(UploadLimit(max_bytes)): Extension<UploadLimit>,
    IdPath(id): IdPath,
    mut multipart: Multipart,
) -> ApiResult<Json<InstalledContentDto>> {
    app.check_content_idle(id)?;
    let mut found: Option<(String, TempUploadGuard)> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(files::from_multipart)?
    {
        if field.name() != Some("file") {
            files::drain_field(field).await?;
            continue;
        }
        if found.is_some() {
            return Err(ApiError::invalid("file"));
        }
        let name = valid_jar_field_name(field.file_name())?;
        let guard = stream_jar_to_temp(&app, field, max_bytes).await?;
        found = Some((name, guard));
    }
    let Some((file_name, guard)) = found else {
        return Err(ApiError::invalid("file"));
    };
    let uploaded = app.upload_content(id, &file_name, &guard.0).await?;
    Ok(Json(uploaded.into()))
}

#[utoipa::path(patch, path = "/api/v1/servers/{id}/content/{file}", tag = "content",
    params(("id" = String, Path), ("file" = String, Path)), request_body = ToggleContentBody,
    responses(
        (status = 204, description = "Đã cập nhật trạng thái"),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
    ))]
pub async fn toggle(
    State(app): State<Arc<App>>,
    ContentFilePath(id, file): ContentFilePath,
    ApiJson(body): ApiJson<ToggleContentBody>,
) -> ApiResult<StatusCode> {
    app.set_content_enabled(id, &file, body.enabled).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/v1/servers/{id}/content/{file}", tag = "content",
    params(("id" = String, Path), ("file" = String, Path)),
    responses(
        (status = 204, description = "Đã chuyển vào thùng rác"),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
    ))]
pub async fn remove(
    State(app): State<Arc<App>>,
    ContentFilePath(id, file): ContentFilePath,
) -> ApiResult<StatusCode> {
    app.remove_content(id, &file).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn check_source(source: &str) -> ApiResult<()> {
    match source {
        SOURCE_MODRINTH => Ok(()),
        "curseforge" => Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::CurseforgeUnavailable,
        )),
        _ => Err(ApiError::invalid("source")),
    }
}

fn parse_kind(raw: &str) -> ApiResult<ProjectType> {
    match raw {
        "mod" => Ok(ProjectType::Mod),
        "plugin" => Ok(ProjectType::Plugin),
        "modpack" => Ok(ProjectType::Modpack),
        _ => Err(ApiError::invalid("kind")),
    }
}

fn validate_install_body(body: &ContentInstallBody) -> ApiResult<(&str, Option<String>)> {
    check_source(&body.source)?;
    let project_id = body.project_id.trim();
    if project_id.is_empty() {
        return Err(ApiError::invalid("project_id"));
    }
    let version_id = match body.version_id.as_deref() {
        Some(v) => {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                return Err(ApiError::invalid("version_id"));
            }
            Some(trimmed.to_owned())
        }
        None => None,
    };
    Ok((project_id, version_id))
}

fn valid_jar_field_name(raw: Option<&str>) -> ApiResult<String> {
    let Some(name) = raw.filter(|n| !n.is_empty()) else {
        return Err(ApiError::invalid("file"));
    };
    let bad = name == "."
        || name == ".."
        || name.chars().any(|c| matches!(c, '/' | '\\' | '\0' | ':'))
        || !Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("jar"));
    if bad {
        return Err(ApiError::invalid("file"));
    }
    Ok(name.to_owned())
}

struct TempUploadGuard(PathBuf);

impl Drop for TempUploadGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

async fn stream_jar_to_temp(
    app: &App,
    mut field: Field<'_>,
    max_bytes: u64,
) -> ApiResult<TempUploadGuard> {
    let cache_dir = app.dirs().cache();
    tokio::fs::create_dir_all(&cache_dir)
        .await
        .map_err(ApiError::internal)?;
    let seq = UPLOAD_SEQ.fetch_add(1, Ordering::Relaxed);
    let temp_path = cache_dir.join(format!(
        ".content-upload-{}-{seq}.jar.part",
        std::process::id()
    ));
    let guard = TempUploadGuard(temp_path.clone());
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(ApiError::internal)?;
    let mut total = 0u64;
    while let Some(chunk) = field.chunk().await.map_err(files::from_multipart)? {
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
