use std::io;
use std::path::PathBuf;

use mh_core::ErrorCode;
use mh_platform::PlatformError;

/// Lỗi nén hoặc giải nén backup.
#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("đã hủy")]
    Cancelled,

    /// Hash file nén không khớp giá trị người gọi đưa vào.
    #[error("file backup sai hash: cần {expected}, nhận {actual}")]
    Corrupt { expected: String, actual: String },

    #[error("đường dẫn `{0}` nằm ngoài thư mục được phép")]
    PathOutsideRoot(String),

    /// Giải nén chỉ được vào thư mục rỗng, để không trộn world cũ với bản backup.
    #[error("thư mục đích `{0}` không rỗng")]
    DestNotEmpty(PathBuf),

    #[error("không đọc được archive: {0}")]
    Archive(String),

    #[error(transparent)]
    Platform(#[from] PlatformError),

    #[error("lỗi I/O tại `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl BackupError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Cancelled => ErrorCode::JobCancelled,
            Self::Corrupt { .. } => ErrorCode::BackupCorrupt,
            Self::PathOutsideRoot(_) => ErrorCode::PathOutsideRoot,
            Self::DestNotEmpty(_) => ErrorCode::InvalidInput,
            Self::Archive(_) | Self::Io { .. } => ErrorCode::Internal,
            Self::Platform(err) => err.code(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_maps_to_backup_corrupt() {
        let err = BackupError::Corrupt {
            expected: "aa".into(),
            actual: "bb".into(),
        };
        assert_eq!(err.code(), ErrorCode::BackupCorrupt);
        assert_eq!(ErrorCode::BackupCorrupt.as_str(), "BACKUP_CORRUPT");
    }

    #[test]
    fn cancelled_maps_to_job_cancelled() {
        assert_eq!(BackupError::Cancelled.code(), ErrorCode::JobCancelled);
    }
}
