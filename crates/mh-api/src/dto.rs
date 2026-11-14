//! Kiểu dữ liệu của API. Tách khỏi kiểu bên trong để thay đổi ở tầng dưới không vô tình đổi API.

use mh_app::jobs::{JobProgress, JobSnapshot, JobState};
use mh_app::{CatalogVersion, MetricSample, ServerMetrics, ServerView};
use mh_core::{ConsoleLine, ConsoleStream, ContentKind, ServerState};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;

use crate::error::ErrorBody;

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StateDto {
    Installing,
    Stopped,
    Starting,
    Running,
    Stopping,
    Crashed,
    Broken,
}

impl From<ServerState> for StateDto {
    fn from(state: ServerState) -> Self {
        match state {
            ServerState::Installing => Self::Installing,
            ServerState::Stopped => Self::Stopped,
            ServerState::Starting => Self::Starting,
            ServerState::Running => Self::Running,
            ServerState::Stopping => Self::Stopping,
            ServerState::Crashed => Self::Crashed,
            ServerState::Broken => Self::Broken,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub struct MemoryDto {
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CrashDto {
    /// Ví dụ `NOT_ENOUGH_MEMORY`; không có nếu không đoán được nguyên nhân.
    pub hint: Option<String>,
    pub exit_code: Option<i32>,
    pub last_lines: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContentKindDto {
    Mods,
    Plugins,
}

impl From<ContentKind> for ContentKindDto {
    fn from(kind: ContentKind) -> Self {
        match kind {
            ContentKind::Mods => Self::Mods,
            ContentKind::Plugins => Self::Plugins,
        }
    }
}

/// Một server trong danh sách. Server đang cài hoặc bị hỏng chỉ có một phần thông tin.
#[derive(Debug, Serialize, ToSchema)]
pub struct ServerDto {
    pub id: String,
    pub state: StateDto,
    pub name: Option<String>,
    pub loader: Option<String>,
    /// `mods`, `plugins`, hoặc `null` với Vanilla / server hỏng.
    pub content_kind: Option<ContentKindDto>,
    pub minecraft: Option<String>,
    pub build: Option<u32>,
    /// Bản mod loader; chỉ có với Fabric, Quilt, Forge, NeoForge.
    pub loader_version: Option<String>,
    pub java_major: Option<u16>,
    /// Bản Java Mojang yêu cầu cho phiên bản Minecraft này; `null` khi chưa biết (spec P3, mục 3.10).
    pub java_recommended: Option<u16>,
    pub port: Option<u16>,
    pub memory_mb: Option<MemoryDto>,
    pub autostart: Option<bool>,
    pub restart_on_crash: Option<bool>,
    /// Giờ khởi động lại hằng ngày (giờ máy), dạng `HH:MM`, đã sắp xếp.
    pub restart_at: Option<Vec<String>>,
    pub backup: Option<BackupSettingsDto>,
    pub eula_accepted: bool,
    /// RFC 3339.
    pub created_at: Option<String>,
    /// Job đang cài server này.
    pub job_id: Option<u64>,
    /// Lý do server hỏng, ví dụ `{ "kind": "newer_schema", "found": 2 }`.
    #[schema(value_type = Option<Object>)]
    pub broken: Option<serde_json::Value>,
    pub crash: Option<CrashDto>,
    pub pid: Option<u32>,
    /// Đã quá 5 phút mà server chưa sẵn sàng.
    pub slow_start: bool,
    /// Đang chờ tự khởi động lại sau crash.
    pub restart_pending: bool,
    /// Có `user_jvm_args.txt` chứa cờ ngoài chú thích mà MineHost bỏ qua.
    pub ignored_jvm_args: bool,
    /// Có lỗi preflight đang chặn bật server (còn thiếu mod hoặc file CurseForge pending).
    pub preflight_blocking: bool,
}

impl From<ServerView> for ServerDto {
    fn from(view: ServerView) -> Self {
        let mut dto = Self {
            id: view.id.to_string(),
            state: view.state.into(),
            name: None,
            loader: None,
            content_kind: None,
            minecraft: None,
            build: None,
            loader_version: None,
            java_major: None,
            java_recommended: None,
            port: None,
            memory_mb: None,
            autostart: None,
            restart_on_crash: None,
            restart_at: None,
            backup: None,
            eula_accepted: false,
            created_at: None,
            job_id: None,
            broken: view.broken.and_then(|b| serde_json::to_value(b).ok()),
            crash: None,
            pid: None,
            slow_start: false,
            restart_pending: false,
            ignored_jvm_args: view.ignored_jvm_args,
            preflight_blocking: view.preflight_blocking,
        };
        if let Some(info) = view.installing {
            dto.name = Some(info.name.to_string());
            dto.loader = Some(info.loader.to_string());
            dto.content_kind = info.loader.content_kind().map(Into::into);
            dto.minecraft = Some(info.minecraft.to_string());
            dto.port = Some(info.port.get());
            dto.job_id = info.job_id;
        }
        if let Some(spec) = view.spec {
            dto.name = Some(spec.name.to_string());
            dto.loader = Some(spec.game.loader.to_string());
            dto.content_kind = spec.game.loader.content_kind().map(Into::into);
            dto.minecraft = Some(spec.game.minecraft.to_string());
            dto.build = spec.game.build;
            dto.loader_version = spec.game.loader_version.as_ref().map(ToString::to_string);
            dto.java_major = Some(spec.game.java_major);
            dto.java_recommended = mh_core::mojang_java_major(&spec.game.minecraft);
            dto.port = Some(spec.network.port.get());
            let memory = spec.runtime.memory_mb;
            dto.memory_mb = Some(MemoryDto {
                min: memory.min_mb(),
                max: memory.max_mb(),
            });
            dto.autostart = Some(spec.policy.autostart);
            dto.restart_on_crash = Some(spec.policy.restart_on_crash);
            dto.restart_at = Some(
                spec.policy
                    .restart_at
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            );
            dto.backup = Some(BackupSettingsDto {
                schedule: spec.backup.schedule.to_string(),
                keep: spec.backup.keep,
            });
            dto.eula_accepted = spec.eula_accepted();
            dto.created_at = spec.created_at.format(&Rfc3339).ok();
        }
        if let Some(status) = view.status {
            dto.pid = status.pid;
            dto.slow_start = status.slow_start;
            dto.restart_pending = status.restart_pending;
            dto.crash = status.crash.map(|c| CrashDto {
                hint: c
                    .hint
                    .and_then(|h| serde_json::to_value(h).ok())
                    .and_then(|v| v.as_str().map(str::to_owned)),
                exit_code: c.exit_code,
                last_lines: c.last_lines.clone(),
            });
        }
        dto
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateServerBody {
    pub name: String,
    /// `paper`, `purpur`, `vanilla`, `fabric`, `quilt`, `forge` hoặc `neoforge`.
    pub loader: String,
    pub minecraft: String,
    /// Chỉ với mod loader; không gửi thì dùng bản đề xuất.
    pub loader_version: Option<String>,
    pub memory_mb: Option<MemoryDto>,
    pub port: Option<u16>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedDto {
    pub server_id: String,
    pub job_id: u64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchServerBody {
    pub name: Option<String>,
    pub memory_mb: Option<MemoryDto>,
    pub port: Option<u16>,
    pub autostart: Option<bool>,
    pub restart_on_crash: Option<bool>,
    /// Giờ khởi động lại hằng ngày, `HH:MM`, tối đa 4 mốc. `[]` là tắt lịch.
    pub restart_at: Option<Vec<String>>,
    /// Lịch sao lưu tự động; trường nào thiếu thì giữ nguyên.
    pub backup: Option<BackupSettingsBody>,
    /// Phiên bản Java chính (`8`, `11`, `17`, `21`, `25`).
    pub java_major: Option<u16>,
}

/// Sao lưu tự động của một server (spec P2, mục 4.1).
#[derive(Debug, Serialize, ToSchema)]
pub struct BackupSettingsDto {
    /// `off`, `every_<n>h` (n từ 1 tới 168) hoặc `daily@HH:MM`.
    pub schedule: String,
    /// Số bản tự động giữ lại, 1..100.
    pub keep: u16,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BackupSettingsBody {
    pub schedule: Option<String>,
    pub keep: Option<u16>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct EulaBody {
    /// Phải là `true`: người dùng bấm "Tôi đồng ý".
    pub accept: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TrashedDto {
    /// Thư mục trong thùng rác, để người dùng tự lấy lại.
    pub trash_path: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CommandBody {
    pub command: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StreamDto {
    Stdout,
    Stderr,
    Stdin,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConsoleLineDto {
    pub seq: u64,
    pub stream: StreamDto,
    pub text: String,
}

impl From<ConsoleLine> for ConsoleLineDto {
    fn from(line: ConsoleLine) -> Self {
        Self {
            seq: line.seq,
            stream: match line.stream {
                ConsoleStream::Stdout => StreamDto::Stdout,
                ConsoleStream::Stderr => StreamDto::Stderr,
                ConsoleStream::Stdin => StreamDto::Stdin,
            },
            text: line.text,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConsoleDto {
    pub lines: Vec<ConsoleLineDto>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PropertyDto {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PropertiesDto {
    /// Theo thứ tự trong file.
    pub properties: Vec<PropertyDto>,
}

/// Một bản mod loader (spec P3, mục 5.8).
#[derive(Debug, Serialize, ToSchema)]
pub struct LoaderVersionDto {
    pub version: String,
    /// Bản ổn định theo nguồn của loader; bản đề xuất luôn đứng đầu danh sách.
    pub stable: bool,
}

impl From<mh_app::sources::LoaderRelease> for LoaderVersionDto {
    fn from(r: mh_app::sources::LoaderRelease) -> Self {
        Self {
            version: r.version.to_string(),
            stable: r.stable,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogVersionDto {
    pub id: String,
    /// RFC 3339.
    pub release_time: String,
    /// Chưa có ở P1: biết được bản Java phải tải JSON của từng phiên bản.
    pub java_major: Option<u16>,
}

impl From<CatalogVersion> for CatalogVersionDto {
    fn from(v: CatalogVersion) -> Self {
        Self {
            id: v.id.to_string(),
            release_time: v.release_time.format(&Rfc3339).unwrap_or_default(),
            java_major: None,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobStateDto {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProgressDto {
    /// Bước đang làm, ví dụ `{ "kind": "download_java", "major": 21 }`.
    #[schema(value_type = Object)]
    pub step: serde_json::Value,
    pub step_index: u8,
    pub step_count: u8,
    pub bytes_done: Option<u64>,
    pub bytes_total: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct JobDto {
    pub id: u64,
    /// Ví dụ `create_server`.
    pub kind: String,
    pub server_id: Option<String>,
    pub state: JobStateDto,
    /// Có khi `state` là `failed`.
    pub error: Option<ErrorBody>,
    pub progress: Option<ProgressDto>,
    /// RFC 3339, lúc Job được tạo.
    pub started_at: String,
    /// RFC 3339, lúc Job sang trạng thái kết thúc. Không có khi còn chạy.
    pub finished_at: Option<String>,
}

impl From<JobSnapshot> for JobDto {
    fn from(job: JobSnapshot) -> Self {
        let (state, error) = match job.state {
            JobState::Running => (JobStateDto::Running, None),
            JobState::Succeeded => (JobStateDto::Succeeded, None),
            JobState::Cancelled => (JobStateDto::Cancelled, None),
            JobState::Failed { code, params } => (
                JobStateDto::Failed,
                Some(ErrorBody {
                    code: code.as_str().to_owned(),
                    params,
                }),
            ),
        };
        let kind = serde_json::to_value(job.kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        Self {
            id: job.id,
            kind,
            server_id: job.server_id.map(|id| id.to_string()),
            state,
            error,
            progress: job.progress.map(progress_dto),
            started_at: rfc3339(job.started_at).unwrap_or_default(),
            finished_at: job.finished_at.and_then(rfc3339),
        }
    }
}

fn rfc3339(t: time::OffsetDateTime) -> Option<String> {
    t.format(&Rfc3339).ok()
}

fn progress_dto(p: JobProgress) -> ProgressDto {
    ProgressDto {
        step: serde_json::to_value(&p.step).unwrap_or_default(),
        step_index: p.step_index,
        step_count: p.step_count,
        bytes_done: p.bytes_done,
        bytes_total: p.bytes_total,
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MetricSampleDto {
    /// RFC 3339.
    pub at: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    /// `null` cho tới khi có theo dõi người chơi.
    pub players: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ServerMetricsDto {
    pub samples: Vec<MetricSampleDto>,
    pub disk_bytes: Option<u64>,
    /// RFC 3339. Có khi server đang ở trạng thái chạy.
    pub running_since: Option<String>,
}

impl From<MetricSample> for MetricSampleDto {
    fn from(sample: MetricSample) -> Self {
        Self {
            at: sample.at.format(&Rfc3339).unwrap_or_default(),
            cpu_percent: sample.cpu_percent,
            memory_bytes: sample.memory_bytes,
            players: sample.players,
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
        }
    }
}
