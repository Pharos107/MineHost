//! Route `/servers/{id}/preflight` và DTO báo cáo kiểm tra trước khi bật (P3 PR-21).

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use mh_app::App;
use mh_app::content::PreflightReport;
use mh_core::{ErrorCode, ModMetaFormat};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::content_dto::PendingContentDto;
use crate::error::{ApiError, ErrorBody, IdPath};

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct StartActionBody {
    #[serde(default)]
    pub ignore_preflight: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MissingDepDto {
    pub id: String,
    pub version_req: String,
    pub required_by: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VersionMismatchDto {
    pub id: String,
    pub version_req: String,
    pub installed_version: String,
    pub required_by: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WrongLoaderDto {
    pub file_name: String,
    pub name: String,
    pub formats: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ClientOnlyDto {
    pub file_name: String,
    pub name: String,
}

/// Báo cáo kiểm tra trước khi bật server.
#[derive(Debug, Serialize, ToSchema)]
pub struct PreflightDto {
    pub missing_deps: Vec<MissingDepDto>,
    pub version_mismatches: Vec<VersionMismatchDto>,
    pub wrong_loader: Vec<WrongLoaderDto>,
    pub pending: Vec<PendingContentDto>,
    pub client_only: Vec<ClientOnlyDto>,
}

impl From<PreflightReport> for PreflightDto {
    fn from(r: PreflightReport) -> Self {
        Self {
            missing_deps: r
                .missing_deps
                .into_iter()
                .map(|d| MissingDepDto {
                    id: d.id,
                    version_req: d.version_req,
                    required_by: d.required_by,
                })
                .collect(),
            version_mismatches: r
                .version_mismatches
                .into_iter()
                .map(|m| VersionMismatchDto {
                    id: m.id,
                    version_req: m.version_req,
                    installed_version: m.installed_version,
                    required_by: m.required_by,
                })
                .collect(),
            wrong_loader: r
                .wrong_loader
                .into_iter()
                .map(|w| WrongLoaderDto {
                    file_name: w.file_name,
                    name: w.name,
                    formats: w.formats.into_iter().map(format_name).collect(),
                })
                .collect(),
            pending: r.pending.into_iter().map(Into::into).collect(),
            client_only: r
                .client_only
                .into_iter()
                .map(|c| ClientOnlyDto {
                    file_name: c.file_name,
                    name: c.name,
                })
                .collect(),
        }
    }
}

fn format_name(fmt: ModMetaFormat) -> String {
    match fmt {
        ModMetaFormat::FabricModJson => "fabric",
        ModMetaFormat::QuiltModJson => "quilt",
        ModMetaFormat::ModsToml => "forge",
        ModMetaFormat::NeoForgeModsToml => "neoforge",
        ModMetaFormat::PluginYml => "plugin",
        ModMetaFormat::PaperPluginYml => "paper-plugin",
    }
    .to_owned()
}

/// Kiểm tra điều kiện mod/plugin trước khi bật server.
#[utoipa::path(
    get,
    path = "/api/v1/servers/{id}/preflight",
    tag = "content",
    params(("id" = String, Path)),
    responses(
        (status = 200, body = PreflightDto),
        (status = 404, body = ErrorBody),
    )
)]
pub async fn get(
    State(app): State<Arc<App>>,
    IdPath(id): IdPath,
) -> Result<Json<PreflightDto>, ApiError> {
    Ok(Json(app.preflight(id).await?.into()))
}

pub(crate) fn parse_optional_start_body(
    headers: &HeaderMap,
    body: &[u8],
) -> Result<StartActionBody, ApiError> {
    if body.iter().all(|b| b.is_ascii_whitespace()) {
        return Ok(StartActionBody::default());
    }
    let is_json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(';').next())
        .map(str::trim)
        .is_some_and(|m| m.eq_ignore_ascii_case("application/json") || m.ends_with("+json"));
    if !is_json {
        return Err(ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorCode::InvalidInput,
        ));
    }
    serde_json::from_slice(body).map_err(|err| {
        tracing::debug!(%err, "body JSON không hợp lệ cho action");
        ApiError::new(StatusCode::BAD_REQUEST, ErrorCode::InvalidInput)
    })
}
