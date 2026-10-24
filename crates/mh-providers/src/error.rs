use mh_core::ErrorCode;
use mh_net::NetError;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error(transparent)]
    Net(#[from] NetError),

    #[error("{source_name} không có phiên bản {version}")]
    VersionNotFound {
        source_name: &'static str,
        version: String,
    },

    /// API trả dữ liệu thiếu hoặc sai (thường là API đổi cấu trúc); chi tiết nằm trong log.
    #[error("dữ liệu từ {source_name} không hợp lệ: {reason}")]
    Invalid {
        source_name: &'static str,
        reason: String,
    },
}

impl ProviderError {
    pub fn code(&self) -> ErrorCode {
        match self {
            // API của Paper trả 404 cho phiên bản không có.
            Self::Net(e) if e.code() == ErrorCode::NotFound => ErrorCode::VersionNotFound,
            Self::Net(e) => e.code(),
            Self::VersionNotFound { .. } => ErrorCode::VersionNotFound,
            Self::Invalid { .. } => ErrorCode::NetworkUnavailable,
        }
    }

    pub(crate) fn invalid(source_name: &'static str, reason: impl Into<String>) -> Self {
        let reason = reason.into();
        tracing::error!(source_name, %reason, "dữ liệu từ API không hợp lệ");
        Self::Invalid {
            source_name,
            reason,
        }
    }
}
