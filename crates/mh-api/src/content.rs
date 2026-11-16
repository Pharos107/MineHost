//! Route tìm kiếm, xem và quản lý mod/plugin (spec P3, mục 5.8).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, FromRequestParts, Multipart, Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use mh_app::App;
use mh_app::content::{
    CONTENT_PAGE_SIZE, CatalogSource, ContentRequest, ContentSearch, ContentSearchResult,
    ContentTarget, MAX_INSTALL_ITEMS, ProjectType, SearchIndex,
};
use mh_core::{LoaderKind, McVersion, ServerId};

use crate::backups::JobStartedDto;
pub(crate) use crate::content_dto::*;
use crate::error::{ApiError, ApiJson, ErrorBody, IdPath};
use crate::files::{self, TempUploadGuard, UploadLimit};

type ApiResult<T> = Result<T, ApiError>;

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
    let source = parse_source(query.source.as_deref().unwrap_or(SOURCE_MODRINTH))?;
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
    let index = query.sort.as_deref().map(parse_sort).transpose()?;
    let search = ContentSearch {
        kind,
        server_id,
        target: parse_target(query.loader.as_deref(), query.minecraft.as_deref())?,
        page,
        index: index.unwrap_or_default(),
    };
    let result: ContentSearchResult = app
        .search_content(source, query.q.as_deref().unwrap_or(""), &search)
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
    let source = parse_source(&source)?;
    let pid = pid.trim();
    if pid.is_empty() {
        return Err(ApiError::invalid("pid"));
    }
    Ok(Json(app.content_project(source, pid).await?.into()))
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
    let source = parse_source(&source)?;
    let pid = pid.trim();
    if pid.is_empty() {
        return Err(ApiError::invalid("pid"));
    }
    let target = parse_target(query.loader.as_deref(), query.minecraft.as_deref())?;
    let list = match (query.server_id.as_deref(), target) {
        (Some(raw_id), _) => {
            let server_id = ServerId::parse(raw_id).map_err(|_| ApiError::invalid("server_id"))?;
            app.content_versions(source, pid, server_id).await?
        }
        (None, Some(target)) => app.target_versions(source, pid, &target).await?,
        (None, None) => app.project_versions(source, pid).await?,
    };
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
    let requests = validate_install_body(&body)?;
    let plans = app.plan_content_requests(id, &requests).await?;
    let kind = match plans.first() {
        Some(plan) => plan.kind,
        None => return Err(ApiError::invalid("items")),
    };
    Ok(Json(ContentPlanDto::merge(kind, plans)))
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
    let requests = validate_install_body(&body)?;
    let job_id = app.install_content_many(id, requests)?;
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
    app.check_content_addable(id)?;
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
        let guard = files::stream_to_temp(&app, "content-upload", "jar", field, max_bytes).await?;
        found = Some((name, guard));
    }
    let Some((file_name, guard)) = found else {
        return Err(ApiError::invalid("file"));
    };
    let Some(temp_path) = guard.path() else {
        return Err(ApiError::invalid("file"));
    };
    let uploaded = app.upload_content(id, &file_name, temp_path).await?;
    Ok(Json(uploaded.into()))
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/content/pending/upload", tag = "content",
    description = "multipart/form-data, một hoặc nhiều phần `file` (.jar ≤ 512 MiB) khớp SHA-1 các mục CurseForge đang chờ.",
    params(("id" = String, Path)),
    responses(
        (status = 200, body = PendingUploadResultDto),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, body = ErrorBody),
        (status = 413, body = ErrorBody),
    ))]
pub async fn upload_pending(
    State(app): State<Arc<App>>,
    Extension(UploadLimit(max_bytes)): Extension<UploadLimit>,
    IdPath(id): IdPath,
    mut multipart: Multipart,
) -> ApiResult<Json<PendingUploadResultDto>> {
    app.check_content_addable(id)?;
    let mut guards: Vec<(String, TempUploadGuard)> = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(files::from_multipart)?
    {
        if field.name() != Some("file") {
            files::drain_field(field).await?;
            continue;
        }
        let name = safe_field_name(field.file_name())?;
        let guard = files::stream_to_temp(&app, "content-upload", "jar", field, max_bytes).await?;
        guards.push((name, guard));
    }
    if guards.is_empty() {
        return Err(ApiError::invalid("file"));
    }
    let items = guards
        .iter()
        .filter_map(|(name, guard)| Some((name.clone(), guard.path()?.to_path_buf())))
        .collect();
    let outcomes = app.upload_pending_batch(id, items).await?;
    let results = outcomes
        .into_iter()
        .map(|item| match item.outcome {
            Ok(installed) => PendingUploadItemDto {
                file_name: item.file_name,
                ok: true,
                installed: Some(installed.into()),
                error: None,
            },
            Err(err) => PendingUploadItemDto {
                file_name: item.file_name,
                ok: false,
                installed: None,
                error: Some(err.into()),
            },
        })
        .collect();
    Ok(Json(PendingUploadResultDto { results }))
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

pub(crate) fn parse_source(source: &str) -> ApiResult<CatalogSource> {
    CatalogSource::parse(source).ok_or_else(|| ApiError::invalid("source"))
}

/// `loader` và `minecraft` đi cùng nhau; thiếu một trong hai là lỗi.
fn parse_target(loader: Option<&str>, minecraft: Option<&str>) -> ApiResult<Option<ContentTarget>> {
    match (loader, minecraft) {
        (None, None) => Ok(None),
        (Some(loader), Some(minecraft)) => Ok(Some(ContentTarget {
            loader: LoaderKind::parse(loader).map_err(|_| ApiError::invalid("loader"))?,
            minecraft: McVersion::parse(minecraft).map_err(|_| ApiError::invalid("minecraft"))?,
        })),
        (None, Some(_)) => Err(ApiError::invalid("loader")),
        (Some(_), None) => Err(ApiError::invalid("minecraft")),
    }
}

fn parse_sort(raw: &str) -> ApiResult<SearchIndex> {
    match raw {
        "relevance" => Ok(SearchIndex::Relevance),
        "downloads" => Ok(SearchIndex::Downloads),
        "updated" => Ok(SearchIndex::Updated),
        _ => Err(ApiError::invalid("sort")),
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

fn validate_install_body(body: &ContentInstallBody) -> ApiResult<Vec<ContentRequest>> {
    if body.items.is_empty() || body.items.len() > MAX_INSTALL_ITEMS {
        return Err(ApiError::invalid("items"));
    }
    body.items.iter().map(validate_item).collect()
}

pub(crate) fn validate_item(item: &ContentItemBody) -> ApiResult<ContentRequest> {
    let source = parse_source(&item.source)?;
    let project_id = item.project_id.trim();
    if project_id.is_empty() {
        return Err(ApiError::invalid("project_id"));
    }
    let version_id = match item.version_id.as_deref().map(str::trim) {
        Some("") => return Err(ApiError::invalid("version_id")),
        Some(v) => Some(v.to_owned()),
        None => None,
    };
    Ok(ContentRequest {
        source,
        project_id: project_id.to_owned(),
        version_id,
    })
}

fn safe_field_name(raw: Option<&str>) -> ApiResult<String> {
    let Some(name) = raw.filter(|n| !n.is_empty()) else {
        return Err(ApiError::invalid("file"));
    };
    let bad =
        name == "." || name == ".." || name.chars().any(|c| matches!(c, '/' | '\\' | '\0' | ':'));
    if bad {
        return Err(ApiError::invalid("file"));
    }
    Ok(name.to_owned())
}

fn valid_jar_field_name(raw: Option<&str>) -> ApiResult<String> {
    let name = safe_field_name(raw)?;
    if !Path::new(&name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("jar"))
    {
        return Err(ApiError::invalid("file"));
    }
    Ok(name)
}
