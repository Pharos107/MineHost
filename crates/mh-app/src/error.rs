use mh_backup::BackupError;
use mh_core::{ErrorCode, InvalidValue, PlayerCommandError};
use mh_java::JavaError;
use mh_net::NetError;
use mh_platform::PlatformError;
use mh_providers::ProviderError;
use mh_runtime::RuntimeError;
use mh_store::StoreError;
use mh_supervisor::SupervisorError;

/// Lỗi của mọi use case: mã lỗi và tham số đi tới client, `detail` chỉ để ghi log.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{code}: {detail}")]
pub struct AppError {
    pub code: ErrorCode,
    pub params: serde_json::Value,
    pub detail: String,
}

impl AppError {
    pub fn new(code: ErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            params: serde_json::Value::Object(Default::default()),
            detail: detail.into(),
        }
    }

    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = params;
        self
    }

    pub(crate) fn cancelled() -> Self {
        Self::new(ErrorCode::JobCancelled, "người dùng hủy")
    }

    pub(crate) fn not_found(what: impl std::fmt::Display) -> Self {
        Self::new(ErrorCode::NotFound, format!("không có {what}"))
    }
}

macro_rules! from_coded {
    ($($ty:ty),* $(,)?) => {$(
        impl From<$ty> for AppError {
            fn from(err: $ty) -> Self {
                Self::new(err.code(), err.to_string())
            }
        }
    )*};
}

from_coded!(
    StoreError,
    ProviderError,
    JavaError,
    NetError,
    PlatformError,
    InvalidValue,
    RuntimeError,
    BackupError,
    PlayerCommandError,
);

impl From<SupervisorError> for AppError {
    fn from(err: SupervisorError) -> Self {
        match err {
            // Giữ tham số (ví dụ cổng gợi ý) mà `Launcher` đã gắn.
            SupervisorError::Launch(e) => Self {
                code: e.code,
                params: e.params,
                detail: e.detail,
            },
            other => Self::new(other.code(), other.to_string()),
        }
    }
}
