//! DTO chỉ số CPU, RAM và người chơi của API.

use mh_app::{MetricSample, ServerMetrics};
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct MetricSampleDto {
    /// RFC 3339.
    pub at: String,
    pub cpu_percent: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_max: Option<f32>,
    pub memory_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_max_bytes: Option<u64>,
    /// `null` cho tới khi có theo dõi người chơi.
    pub players: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub players_max: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ServerMetricsDto {
    pub samples: Vec<MetricSampleDto>,
    pub disk_bytes: Option<u64>,
    /// RFC 3339. Có khi server đang ở trạng thái chạy.
    pub running_since: Option<String>,
    /// Thời điểm mẫu cũ nhất đang giữ (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oldest_at: Option<String>,
    /// Thời gian giữ lịch sử chỉ số đang đặt cho server (phút).
    pub retention_minutes: u32,
    /// Bước thời gian giữa các điểm tính bằng giây.
    pub step_seconds: u32,
}

impl From<MetricSample> for MetricSampleDto {
    fn from(sample: MetricSample) -> Self {
        Self {
            at: sample.at.format(&Rfc3339).unwrap_or_default(),
            cpu_percent: sample.cpu_percent,
            cpu_max: sample.cpu_max,
            memory_bytes: sample.memory_bytes,
            memory_max_bytes: sample.memory_max_bytes,
            players: sample.players,
            players_max: sample.players_max,
        }
    }
}

impl From<ServerMetrics> for ServerMetricsDto {
    fn from(metrics: ServerMetrics) -> Self {
        Self {
            samples: metrics.samples.into_iter().map(Into::into).collect(),
            disk_bytes: metrics.disk_bytes,
            running_since: metrics
                .running_since
                .and_then(|at| at.format(&Rfc3339).ok()),
            oldest_at: metrics.oldest_at.and_then(|at| at.format(&Rfc3339).ok()),
            retention_minutes: metrics.retention_minutes,
            step_seconds: metrics.step_seconds,
        }
    }
}
