//! `GET`/`PUT /settings/access`: công tắc mở giao diện ra LAN (spec P2, mục 5.8, 6.10).

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use mh_app::App;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::ApiError;
use crate::error::{ApiJson, ErrorBody};
use crate::lan::LanState;

/// Chờ binary mở listener tối đa chừng này rồi trả kết quả, để UI hiện được địa chỉ ngay.
const APPLY_WAIT: Duration = Duration::from_secs(3);

#[derive(Debug, Serialize, ToSchema)]
pub struct AccessDto {
    /// Cờ trong `auth.toml`. Luôn `false` khi chưa đặt mật khẩu.
    pub lan: bool,
    /// Địa chỉ mở được từ máy khác, ví dụ `http://192.168.1.5:7878`. Rỗng khi LAN tắt hoặc
    /// chưa mở được listener nào (máy không có mạng LAN, cổng bị chiếm).
    pub lan_urls: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AccessBody {
    pub lan: bool,
}

#[utoipa::path(get, path = "/api/v1/settings/access", tag = "settings",
    responses((status = 200, body = AccessDto), (status = 401, body = ErrorBody)))]
pub async fn get_access(
    State(app): State<Arc<App>>,
    State(lan): State<Arc<LanState>>,
) -> Result<Json<AccessDto>, ApiError> {
    Ok(Json(access(&app, &lan)?))
}

#[utoipa::path(put, path = "/api/v1/settings/access", tag = "settings", request_body = AccessBody,
    responses(
        (status = 200, body = AccessDto),
        (status = 401, body = ErrorBody),
        (status = 409, description = "Bật LAN khi chưa có mật khẩu", body = ErrorBody)
    ))]
pub async fn put_access(
    State(app): State<Arc<App>>,
    State(lan): State<Arc<LanState>>,
    ApiJson(body): ApiJson<AccessBody>,
) -> Result<Json<AccessDto>, ApiError> {
    app.auth().set_lan(body.lan)?;
    lan.apply_now(APPLY_WAIT).await;
    Ok(Json(access(&app, &lan)?))
}

fn access(app: &App, lan: &LanState) -> Result<AccessDto, ApiError> {
    let on = app.auth().lan()?;
    let lan_urls = if on {
        lan.open_addrs()
            .iter()
            .map(|addr| format!("http://{addr}"))
            .collect()
    } else {
        Vec::new()
    };
    Ok(AccessDto { lan: on, lan_urls })
}
