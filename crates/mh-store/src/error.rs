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
}

impl StoreError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::Broken { .. } => ErrorCode::ConfigInvalid,
            Self::Serialize(e) => e.code(),
            Self::Platform(e) => e.code(),
            Self::AlreadyExists(_) | Self::Io { .. } => ErrorCode::Internal,
        }
    }
}
