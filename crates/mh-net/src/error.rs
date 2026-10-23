use std::io;
use std::path::PathBuf;

use mh_core::ErrorCode;

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    /// Lỗi kết nối, timeout hoặc 5xx/429 sau khi đã hết lượt retry.
    #[error("không gọi được {url}: {reason}")]
    Unavailable { url: String, reason: String },

    /// HTTP lỗi không đáng retry, ví dụ 404.
    #[error("{url} trả về HTTP {status}")]
    Status { url: String, status: u16 },

    /// API trả dữ liệu không đúng định dạng mong đợi (thường là API đổi cấu trúc).
    #[error("dữ liệu từ {url} không đúng định dạng: {reason}")]
    Decode { url: String, reason: String },

    #[error("tải {url} bị lỗi giữa chừng: {reason}")]
    Download { url: String, reason: String },

    #[error("file tải từ {url} sai hash: cần {expected}, nhận {actual}")]
    ChecksumMismatch {
        url: String,
        expected: String,
        actual: String,
    },

    #[error("đã hủy")]
    Cancelled,

    #[error("ổ đĩa chỉ còn {available} byte trống")]
    NotEnoughDisk { available: u64 },

    #[error("lỗi I/O tại `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl NetError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Unavailable { .. } | Self::Decode { .. } => ErrorCode::NetworkUnavailable,
            Self::Status { status: 404, .. } => ErrorCode::NotFound,
            Self::Status { .. } => ErrorCode::NetworkUnavailable,
            Self::Download { .. } => ErrorCode::DownloadFailed,
            Self::ChecksumMismatch { .. } => ErrorCode::ChecksumMismatch,
            Self::Cancelled => ErrorCode::JobCancelled,
            Self::NotEnoughDisk { .. } => ErrorCode::NotEnoughDisk,
            Self::Io { .. } => ErrorCode::Internal,
        }
    }

    /// Lỗi do mạng chứ không do dữ liệu: dùng bản cache cũ thay thế được.
    pub(crate) fn is_network(&self) -> bool {
        match self {
            Self::Unavailable { .. } => true,
            Self::Status { status, .. } => *status >= 500 || *status == 429,
            _ => false,
        }
    }
}
