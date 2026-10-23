use std::io;
use std::path::PathBuf;

use mh_core::ErrorCode;

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("không xác định được thư mục dữ liệu của người dùng")]
    NoHomeDir,

    #[error("đường dẫn `{0}` nằm ngoài thư mục được phép")]
    PathOutsideRoot(String),

    #[error("đường dẫn `{0}` không hợp lệ")]
    InvalidPath(String),

    #[error("lỗi I/O tại `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl PlatformError {
    pub fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            Self::NoHomeDir | Self::Io { .. } => ErrorCode::Internal,
            Self::PathOutsideRoot(_) => ErrorCode::PathOutsideRoot,
            Self::InvalidPath(_) => ErrorCode::InvalidInput,
        }
    }
}
