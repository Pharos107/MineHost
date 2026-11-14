//! Route tạo server từ modpack (`/servers/modpack` và `/servers/modpack/upload`, spec P3, mục 5.8).

use std::path::Path;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Multipart, State};
use axum::http::StatusCode;
use mh_app::{App, ModpackRequest, ModpackSourceInput};
use mh_core::{ErrorCode, GamePort, MemoryLimits, ServerName};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::content::check_source;
use crate::dto::{CreatedDto, MemoryDto};
use crate::error::{ApiError, ApiJson, ErrorBody};
use crate::files::{self, TempUploadGuard, UploadLimit};
use crate::servers::{memory, port};

type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateModpackBody {
    /// `modrinth`.
    pub source: String,
    pub project_id: String,
    pub version_id: Option<String>,
    pub name: String,
    pub memory_mb: Option<MemoryDto>,
    pub port: Option<u16>,
}

#[utoipa::path(post, path = "/api/v1/servers/modpack", tag = "servers", request_body = CreateModpackBody,
    responses(
        (status = 202, description = "Job cài modpack đã chạy", body = CreatedDto),
        (status = 400, body = ErrorBody),
        (status = 409, body = ErrorBody),
        (status = 503, body = ErrorBody),
    ))]
pub async fn create(
    State(app): State<Arc<App>>,
    ApiJson(body): ApiJson<CreateModpackBody>,
) -> ApiResult<(StatusCode, Json<CreatedDto>)> {
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
    let req = ModpackRequest {
        source: ModpackSourceInput::Modrinth {
            project_id: project_id.to_owned(),
            version_id,
        },
        name: ServerName::parse(&body.name).map_err(|_| ApiError::invalid("name"))?,
        memory_mb: body.memory_mb.map(memory).transpose()?,
        port: body.port.map(port).transpose()?,
    };
    let created = app.create_from_modpack(req)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(CreatedDto {
            server_id: created.server_id.to_string(),
            job_id: created.job_id,
        }),
    ))
}

#[utoipa::path(post, path = "/api/v1/servers/modpack/upload", tag = "servers",
    description = "multipart/form-data, một file .mrpack tối đa 512 MiB kèm trường name (tùy chọn memory_mb, port).",
    responses(
        (status = 202, description = "Job cài modpack đã chạy", body = CreatedDto),
        (status = 400, body = ErrorBody),
        (status = 409, body = ErrorBody),
        (status = 413, body = ErrorBody),
        (status = 422, body = ErrorBody),
    ))]
pub async fn upload(
    State(app): State<Arc<App>>,
    Extension(UploadLimit(max_bytes)): Extension<UploadLimit>,
    multipart: Multipart,
) -> ApiResult<(StatusCode, Json<CreatedDto>)> {
    let form = read_upload_form(&app, max_bytes, multipart).await?;
    let Some((original_name, guard)) = form.file else {
        return Err(ApiError::invalid("file"));
    };
    let Some(raw_name) = form.name else {
        return Err(ApiError::invalid("name"));
    };
    let name = ServerName::parse(&raw_name).map_err(|_| ApiError::invalid("name"))?;
    let Some(path) = guard.path().map(Path::to_path_buf) else {
        return Err(ApiError::invalid("file"));
    };
    let req = ModpackRequest {
        source: ModpackSourceInput::UploadedFile {
            path,
            original_name,
        },
        name,
        memory_mb: form.memory_mb,
        port: form.port,
    };
    let created = app.create_from_modpack(req)?;
    // Chỉ nhường quyền xóa file tạm cho Job khi `create_from_modpack` đã nhận chạy.
    let _ = guard.disarm();
    Ok((
        StatusCode::ACCEPTED,
        Json(CreatedDto {
            server_id: created.server_id.to_string(),
            job_id: created.job_id,
        }),
    ))
}

struct UploadForm {
    file: Option<(String, TempUploadGuard)>,
    name: Option<String>,
    memory_mb: Option<MemoryLimits>,
    port: Option<GamePort>,
}

async fn read_upload_form(
    app: &App,
    max_bytes: u64,
    mut multipart: Multipart,
) -> ApiResult<UploadForm> {
    let mut form = UploadForm {
        file: None,
        name: None,
        memory_mb: None,
        port: None,
    };
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(files::from_multipart)?
    {
        match field.name() {
            Some("file") => {
                if form.file.is_some() {
                    return Err(ApiError::invalid("file"));
                }
                let file_name = validate_pack_filename(field.file_name())?;
                let guard =
                    files::stream_to_temp(app, "modpack-upload", "mrpack", field, max_bytes)
                        .await?;
                form.file = Some((file_name, guard));
            }
            Some("name") => {
                let text = field.text().await.map_err(|_| ApiError::invalid("name"))?;
                form.name = Some(text);
            }
            Some("memory_mb") => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| ApiError::invalid("memory_mb"))?;
                form.memory_mb = parse_memory_field(&text)?;
            }
            Some("port") => {
                let text = field.text().await.map_err(|_| ApiError::invalid("port"))?;
                form.port = parse_port_field(&text)?;
            }
            _ => files::drain_field(field).await?,
        }
    }
    Ok(form)
}

fn validate_pack_filename(raw: Option<&str>) -> ApiResult<String> {
    let Some(name) = raw.filter(|n| !n.is_empty()) else {
        return Err(ApiError::invalid("file"));
    };
    let bad =
        name == "." || name == ".." || name.chars().any(|c| matches!(c, '/' | '\\' | '\0' | ':'));
    if bad {
        return Err(ApiError::invalid("file"));
    }
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("mrpack") => Ok(name.to_owned()),
        Some("zip") => Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::ModpackInvalid,
        )
        .with_params(serde_json::json!({
            "reason": "CurseForge .zip chưa được hỗ trợ ở phiên bản này (chỉ nhận .mrpack)"
        }))),
        _ => Err(
            ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, ErrorCode::ModpackInvalid).with_params(
                serde_json::json!({
                    "reason": "chỉ hỗ trợ file modpack .mrpack"
                }),
            ),
        ),
    }
}

fn parse_memory_field(raw: &str) -> ApiResult<Option<MemoryLimits>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.starts_with('{') {
        let dto: MemoryDto =
            serde_json::from_str(trimmed).map_err(|_| ApiError::invalid("memory_mb"))?;
        return Ok(Some(memory(dto)?));
    }
    let max: u32 = trimmed
        .parse()
        .map_err(|_| ApiError::invalid("memory_mb"))?;
    let limits =
        MemoryLimits::new(max.min(1024), max).map_err(|_| ApiError::invalid("memory_mb"))?;
    Ok(Some(limits))
}

fn parse_port_field(raw: &str) -> ApiResult<Option<GamePort>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let p: u16 = trimmed.parse().map_err(|_| ApiError::invalid("port"))?;
    Ok(Some(port(p)?))
}
