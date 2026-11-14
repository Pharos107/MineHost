use std::io;
use std::path::PathBuf;

use mh_core::ErrorCode;
use mh_net::NetError;
use mh_platform::PlatformError;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error(transparent)]
    Net(#[from] NetError),

    #[error(transparent)]
    Platform(#[from] PlatformError),

    #[error("lỗi I/O tại `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("{source_name} không có phiên bản {version}")]
    VersionNotFound {
        source_name: &'static str,
        version: String,
    },

    /// Project hoặc file không còn trên Modrinth hay CurseForge (spec P3, mục 4.4).
    #[error("{source_name} không có nội dung {id}")]
    ContentNotFound {
        source_name: &'static str,
        id: String,
    },

    /// API trả dữ liệu thiếu hoặc sai (thường là API đổi cấu trúc); chi tiết nằm trong log.
    #[error("dữ liệu từ {source_name} không hợp lệ: {reason}")]
    Invalid {
        source_name: &'static str,
        reason: String,
    },

    /// Mojang không có tài khoản đúng tên này. Khác với phiên bản server không tồn tại.
    #[error("không có tài khoản Minecraft tên {0}")]
    PlayerNotFound(String),

    /// Hồ sơ không có ảnh. App dùng skin mặc định; không phải `PLAYER_NOT_FOUND`.
    #[error("không có skin")]
    NoSkin,

    /// File modpack thiếu index, sai định dạng, vượt giới hạn hoặc có đường dẫn thoát thư mục.
    #[error("modpack không hợp lệ: {reason}")]
    ModpackInvalid { reason: String },
}

impl ProviderError {
    /// Tham số đi kèm mã lỗi; chỉ lỗi mạng mới có (ví dụ `retry_after_s`).
    pub fn params(&self) -> serde_json::Value {
        match self {
            Self::Net(e) => e.params(),
            _ => serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            // API của Paper trả 404 cho phiên bản không có. 404 của hồ sơ người chơi
            // được đổi thành `PlayerNotFound` trước khi tới đây.
            Self::Net(e) if e.code() == ErrorCode::NotFound => ErrorCode::VersionNotFound,
            Self::Net(e) => e.code(),
            Self::Platform(e) => e.code(),
            Self::Io { .. } => ErrorCode::Internal,
            Self::VersionNotFound { .. } => ErrorCode::VersionNotFound,
            Self::ContentNotFound { .. } => ErrorCode::ContentNotFound,
            Self::Invalid { .. } => ErrorCode::NetworkUnavailable,
            Self::PlayerNotFound(_) => ErrorCode::PlayerNotFound,
            // Người gọi phải khớp biến thể này để lấy skin mặc định, không dựa vào mã HTTP.
            Self::NoSkin => ErrorCode::NotFound,
            Self::ModpackInvalid { .. } => ErrorCode::ModpackInvalid,
        }
    }

    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn modpack_invalid(reason: impl Into<String>) -> Self {
        Self::ModpackInvalid {
            reason: reason.into(),
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
