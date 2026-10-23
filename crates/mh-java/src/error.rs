use mh_core::ErrorCode;
use mh_net::NetError;

#[derive(Debug, thiserror::Error)]
pub enum JavaError {
    #[error(transparent)]
    Net(#[from] NetError),

    #[error("Adoptium không có JRE {major} cho {os}/{arch}")]
    NotAvailable {
        major: u16,
        os: &'static str,
        arch: &'static str,
    },

    /// Giải nén hoặc chạy thử `java -version` thất bại.
    #[error("không cài được Java: {0}")]
    Install(String),

    /// Archive có đường dẫn trỏ ra ngoài thư mục giải nén (zip-slip).
    #[error("file nén Java có đường dẫn không an toàn: {0}")]
    UnsafeArchive(String),
}

impl JavaError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Net(e) => e.code(),
            Self::NotAvailable { .. } | Self::Install(_) | Self::UnsafeArchive(_) => {
                ErrorCode::JavaInstallFailed
            }
        }
    }

    pub(crate) fn install(reason: impl std::fmt::Display) -> Self {
        Self::Install(reason.to_string())
    }
}
