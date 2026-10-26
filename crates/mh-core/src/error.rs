use std::fmt;

use serde::{Deserialize, Serialize};

/// Mã lỗi cố định gửi cho client.
///
/// Frontend dịch thông báo theo mã này, nên không được đổi tên một mã đã phát hành.
/// Thêm mã mới thì thêm cả bản dịch trong `ui/src/i18n/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum ErrorCode {
    /// Lỗi không lường trước; chi tiết chỉ nằm trong log.
    Internal,
    NotFound,
    /// Không có tài khoản Minecraft đúng tên này trên Mojang (server bật online-mode).
    PlayerNotFound,
    InvalidInput,
    /// Đường dẫn do người dùng gửi lên trỏ ra ngoài thư mục được phép.
    PathOutsideRoot,
    PortInUse,
    ConfigInvalid,
    /// Thao tác không hợp lệ với trạng thái hiện tại của server.
    ServerBusy,
    EulaNotAccepted,
    /// Phiên bản không có trên Mojang hoặc Paper.
    VersionNotFound,
    /// Không gọi được API sau khi đã retry.
    NetworkUnavailable,
    DownloadFailed,
    ChecksumMismatch,
    /// Giải nén hoặc chạy thử Java thất bại.
    JavaInstallFailed,
    NotEnoughDisk,
    JobCancelled,
    /// Server thoát bất thường; chi tiết nằm trong `hint`.
    ServerCrashed,
    /// Request không đến từ giao diện MineHost trên máy này (sai Host hoặc Origin).
    Forbidden,
    /// Tính năng không có ở phiên bản Minecraft này, ví dụ đóng băng trước 1.16.
    UnsupportedByVersion,
    /// File văn bản hoặc bản tải lên vượt giới hạn kích thước.
    FileTooLarge,
    /// Không phải UTF-8 hợp lệ, hoặc có byte NUL.
    FileNotText,
    /// `If-Match` không khớp nội dung hiện trên đĩa.
    FileChanged,
    /// Tạo, tải lên hoặc đổi tên đè lên mục đã có.
    FileExists,
    /// Hash của file backup không khớp với file mô tả.
    BackupCorrupt,
}

impl ErrorCode {
    /// Chuỗi giống hệt giá trị serde, dùng cho log và thông báo dòng lệnh.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "INTERNAL",
            Self::NotFound => "NOT_FOUND",
            Self::PlayerNotFound => "PLAYER_NOT_FOUND",
            Self::InvalidInput => "INVALID_INPUT",
            Self::PathOutsideRoot => "PATH_OUTSIDE_ROOT",
            Self::PortInUse => "PORT_IN_USE",
            Self::ConfigInvalid => "CONFIG_INVALID",
            Self::ServerBusy => "SERVER_BUSY",
            Self::EulaNotAccepted => "EULA_NOT_ACCEPTED",
            Self::VersionNotFound => "VERSION_NOT_FOUND",
            Self::NetworkUnavailable => "NETWORK_UNAVAILABLE",
            Self::DownloadFailed => "DOWNLOAD_FAILED",
            Self::ChecksumMismatch => "CHECKSUM_MISMATCH",
            Self::JavaInstallFailed => "JAVA_INSTALL_FAILED",
            Self::NotEnoughDisk => "NOT_ENOUGH_DISK",
            Self::JobCancelled => "JOB_CANCELLED",
            Self::ServerCrashed => "SERVER_CRASHED",
            Self::Forbidden => "FORBIDDEN",
            Self::UnsupportedByVersion => "UNSUPPORTED_BY_VERSION",
            Self::FileTooLarge => "FILE_TOO_LARGE",
            Self::FileNotText => "FILE_NOT_TEXT",
            Self::FileChanged => "FILE_CHANGED",
            Self::FileExists => "FILE_EXISTS",
            Self::BackupCorrupt => "BACKUP_CORRUPT",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Giá trị không qua được hàm kiểm tra của một kiểu domain.
///
/// `kind` và `reason` là chuỗi tĩnh để log dễ lọc; client chỉ nhận `INVALID_INPUT`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind} không hợp lệ: {reason}")]
pub struct InvalidValue {
    pub kind: &'static str,
    pub reason: &'static str,
}

impl InvalidValue {
    pub(crate) const fn new(kind: &'static str, reason: &'static str) -> Self {
        Self { kind, reason }
    }

    pub const fn code(&self) -> ErrorCode {
        ErrorCode::InvalidInput
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [ErrorCode; 24] = [
        ErrorCode::Internal,
        ErrorCode::NotFound,
        ErrorCode::PlayerNotFound,
        ErrorCode::InvalidInput,
        ErrorCode::PathOutsideRoot,
        ErrorCode::PortInUse,
        ErrorCode::ConfigInvalid,
        ErrorCode::ServerBusy,
        ErrorCode::EulaNotAccepted,
        ErrorCode::VersionNotFound,
        ErrorCode::NetworkUnavailable,
        ErrorCode::DownloadFailed,
        ErrorCode::ChecksumMismatch,
        ErrorCode::JavaInstallFailed,
        ErrorCode::NotEnoughDisk,
        ErrorCode::JobCancelled,
        ErrorCode::ServerCrashed,
        ErrorCode::Forbidden,
        ErrorCode::UnsupportedByVersion,
        ErrorCode::FileTooLarge,
        ErrorCode::FileNotText,
        ErrorCode::FileChanged,
        ErrorCode::FileExists,
        ErrorCode::BackupCorrupt,
    ];

    #[test]
    fn as_str_matches_serde_name() {
        for code in ALL {
            let json = serde_json::to_string(&code).unwrap();
            assert_eq!(json, format!("\"{}\"", code.as_str()));
        }
    }

    #[test]
    fn round_trips_through_json() {
        for code in ALL {
            let json = serde_json::to_string(&code).unwrap();
            let back: ErrorCode = serde_json::from_str(&json).unwrap();
            assert_eq!(back, code);
        }
    }

    /// Mỗi mã phải có bản dịch, nếu không UI hiện nguyên mã cho người dùng.
    #[test]
    fn every_code_has_translations() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/i18n");
        for lang in ["vi", "en"] {
            let path = format!("{root}/{lang}.json");
            let text = std::fs::read_to_string(&path).unwrap();
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            for code in ALL {
                assert!(
                    json["errors"][code.as_str()].is_string(),
                    "{path} thiếu errors.{code}"
                );
            }
        }
    }
}
