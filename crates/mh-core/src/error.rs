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
    InvalidInput,
    /// Đường dẫn do người dùng gửi lên trỏ ra ngoài thư mục được phép.
    PathOutsideRoot,
    PortInUse,
    ConfigInvalid,
}

impl ErrorCode {
    /// Chuỗi giống hệt giá trị serde, dùng cho log và thông báo dòng lệnh.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "INTERNAL",
            Self::NotFound => "NOT_FOUND",
            Self::InvalidInput => "INVALID_INPUT",
            Self::PathOutsideRoot => "PATH_OUTSIDE_ROOT",
            Self::PortInUse => "PORT_IN_USE",
            Self::ConfigInvalid => "CONFIG_INVALID",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [ErrorCode; 6] = [
        ErrorCode::Internal,
        ErrorCode::NotFound,
        ErrorCode::InvalidInput,
        ErrorCode::PathOutsideRoot,
        ErrorCode::PortInUse,
        ErrorCode::ConfigInvalid,
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
}
