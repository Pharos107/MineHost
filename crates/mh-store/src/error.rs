use std::io;
use std::path::PathBuf;

use mh_core::{ErrorCode, InstanceError, ServerId};
use mh_platform::PlatformError;

use crate::BrokenReason;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("không có server {0}")]
    NotFound(ServerId),

    /// Trùng ULID; gần như không thể xảy ra, trừ khi đồng hồ máy bị chỉnh lùi.
    #[error("thư mục server {0} đã tồn tại")]
    AlreadyExists(ServerId),

    #[error("server {id} hỏng: {reason}")]
    Broken { id: ServerId, reason: BrokenReason },

    #[error(transparent)]
    Serialize(InstanceError),

    #[error(transparent)]
    Platform(#[from] PlatformError),

    #[error("lỗi I/O tại `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// `whitelist.json` và các file người chơi không phải JSON đúng dạng Minecraft ghi.
    #[error("file người chơi `{path}` không hợp lệ: {detail}")]
    InvalidPlayers { path: PathBuf, detail: String },
}

impl StoreError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn invalid_players(path: impl Into<PathBuf>, err: impl std::fmt::Display) -> Self {
        Self::InvalidPlayers {
            path: path.into(),
            detail: err.to_string(),
        }
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            Self::NotFound(_) => ErrorCode::NotFound,
            // File người chơi hỏng cùng loại với instance.toml hỏng: người dùng sửa được file.
            Self::Broken { .. } | Self::InvalidPlayers { .. } => ErrorCode::ConfigInvalid,
            Self::Serialize(e) => e.code(),
            Self::Platform(e) => e.code(),
            Self::AlreadyExists(_) | Self::Io { .. } => ErrorCode::Internal,
        }
    }
}
