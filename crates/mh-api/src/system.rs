use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use mh_app::App;
use serde::Serialize;
use utoipa::ToSchema;

use crate::ApiError;
use crate::dto::MemoryDto;
use crate::error::ErrorBody;

#[derive(Debug, Serialize, ToSchema)]
pub struct Health {
    pub status: &'static str,
    pub version: &'static str,
}

/// Thông tin máy gửi cho UI. Tách khỏi `mh_platform::SystemInfo` để thay đổi bên trong
/// không vô tình làm đổi API.
#[derive(Debug, Serialize, ToSchema)]
pub struct SystemDto {
    pub version: &'static str,
    pub os: String,
    pub os_version: Option<String>,
    pub arch: String,
    pub cpu_brand: String,
    pub cpu_threads: usize,
    pub memory_total_bytes: u64,
    pub memory_available_bytes: u64,
    pub data_dir: String,
    pub data_disk: Option<DiskDto>,
    /// RAM đề xuất cho server mới (spec P1, mục 4.3).
    pub suggested_memory_mb: MemoryDto,
    /// Địa chỉ IPv4 trong mạng LAN, để bạn bè vào chơi.
    pub lan_addresses: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DiskDto {
    pub mount_point: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[utoipa::path(
    get,
    path = "/api/v1/health",
    tag = "system",
    responses((status = 200, description = "Controller đang chạy", body = Health))
)]
pub async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: mh_app::VERSION,
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/system",
    tag = "system",
    responses(
        (status = 200, description = "RAM, CPU và ổ đĩa của máy", body = SystemDto),
        (status = 500, description = "Lỗi nội bộ", body = ErrorBody)
    )
)]
pub async fn system_info(State(app): State<Arc<App>>) -> Result<Json<SystemDto>, ApiError> {
    let data_dir = app.dirs().root().display().to_string();
    let suggested = app.suggested_memory();
    let lan_addresses = app
        .lan_addresses()
        .iter()
        .map(ToString::to_string)
        .collect();
    let info = tokio::task::spawn_blocking(move || app.system_info())
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(SystemDto {
        version: mh_app::VERSION,
        os: info.os,
        os_version: info.os_version,
        arch: info.arch,
        cpu_brand: info.cpu_brand,
        cpu_threads: info.cpu_threads,
        memory_total_bytes: info.memory_total_bytes,
        memory_available_bytes: info.memory_available_bytes,
        data_dir,
        data_disk: info.data_disk.map(|disk| DiskDto {
            mount_point: disk.mount_point.display().to_string(),
            total_bytes: disk.total_bytes,
            available_bytes: disk.available_bytes,
        }),
        suggested_memory_mb: MemoryDto {
            min: suggested.min_mb(),
            max: suggested.max_mb(),
        },
        lan_addresses,
    }))
}
