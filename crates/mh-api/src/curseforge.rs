//! `GET`/`PUT`/`DELETE /settings/curseforge`: quản lý API key CurseForge (spec P3, mục 5.8).

use std::fmt;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use mh_app::{App, CurseforgeKeySource, CurseforgeStatus};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::ApiError;
use crate::error::{ApiJson, ErrorBody};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CurseforgeKeySourceDto {
    Builtin,
    User,
    None,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CurseforgeSettingsDto {
    pub available: bool,
    pub source: CurseforgeKeySourceDto,
}

#[derive(Deserialize, ToSchema)]
pub struct CurseforgeKeyBody {
    pub api_key: String,
}

impl fmt::Debug for CurseforgeKeyBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Không in key gửi lên vì `Debug` có thể đi vào log.
        f.debug_struct("CurseforgeKeyBody")
            .field("api_key", &"***")
            .finish()
    }
}

impl From<CurseforgeStatus> for CurseforgeSettingsDto {
    fn from(status: CurseforgeStatus) -> Self {
        Self {
            available: status.available,
            source: match status.source {
                CurseforgeKeySource::Builtin => CurseforgeKeySourceDto::Builtin,
                CurseforgeKeySource::User => CurseforgeKeySourceDto::User,
                CurseforgeKeySource::None => CurseforgeKeySourceDto::None,
            },
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/curseforge",
    tag = "settings",
    responses(
        (status = 200, body = CurseforgeSettingsDto),
        (status = 401, body = ErrorBody),
        (status = 422, body = ErrorBody)
    )
)]
pub async fn get_curseforge(
    State(app): State<Arc<App>>,
) -> Result<Json<CurseforgeSettingsDto>, ApiError> {
    Ok(Json(app.curseforge_status()?.into()))
}

#[utoipa::path(
    put,
    path = "/api/v1/settings/curseforge",
    tag = "settings",
    request_body = CurseforgeKeyBody,
    responses(
        (status = 200, body = CurseforgeSettingsDto),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 422, body = ErrorBody)
    )
)]
pub async fn put_curseforge(
    State(app): State<Arc<App>>,
    ApiJson(body): ApiJson<CurseforgeKeyBody>,
) -> Result<Json<CurseforgeSettingsDto>, ApiError> {
    Ok(Json(app.set_curseforge_key(&body.api_key)?.into()))
}

#[utoipa::path(
    delete,
    path = "/api/v1/settings/curseforge",
    tag = "settings",
    responses(
        (status = 200, body = CurseforgeSettingsDto),
        (status = 401, body = ErrorBody),
        (status = 422, body = ErrorBody)
    )
)]
pub async fn delete_curseforge(
    State(app): State<Arc<App>>,
) -> Result<Json<CurseforgeSettingsDto>, ApiError> {
    Ok(Json(app.clear_curseforge_key()?.into()))
}
