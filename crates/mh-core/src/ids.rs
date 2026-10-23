//! Định danh server, tên hiển thị và cổng game.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::error::InvalidValue;

/// ID của server, dạng ULID 26 ký tự viết hoa.
///
/// ID được ghép vào đường dẫn `servers/<id>/`, nên chỉ tạo được từ ULID hợp lệ:
/// chuỗi lạ từ client không bao giờ thành một đoạn đường dẫn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ServerId(Ulid);

impl ServerId {
    /// Tạo ID mới theo đồng hồ và bộ sinh số ngẫu nhiên của máy.
    pub fn generate() -> Self {
        Self(Ulid::generate())
    }

    /// Nhận cả chữ thường; khi hiển thị luôn viết hoa để khớp tên thư mục.
    pub fn parse(s: &str) -> Result<Self, InvalidValue> {
        let err = InvalidValue::new("ServerId", "không phải ULID");
        let ulid = Ulid::from_string(s).map_err(|_| err.clone())?;
        // `ulid` chấp nhận cả chuỗi tràn 128 bit và chữ thay thế kiểu Crockford (`O` thành `0`).
        // Bắt buộc in ra đúng chuỗi đã nhận, để một thư mục chỉ có đúng một cách viết ID.
        if !ulid.to_string().eq_ignore_ascii_case(s) {
            return Err(err);
        }
        Ok(Self(ulid))
    }
}

impl fmt::Display for ServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for ServerId {
    type Err = InvalidValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for ServerId {
    type Error = InvalidValue;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<ServerId> for String {
    fn from(id: ServerId) -> Self {
        id.to_string()
    }
}

/// Tên server người dùng đặt: 1–64 ký tự sau khi bỏ khoảng trắng hai đầu, không có ký tự điều khiển.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ServerName(String);

impl ServerName {
    pub const MAX_CHARS: usize = 64;

    pub fn parse(s: &str) -> Result<Self, InvalidValue> {
        let s = s.trim();
        if s.is_empty() {
            return Err(InvalidValue::new("ServerName", "rỗng"));
        }
        // Đếm theo ký tự chứ không theo byte: tên tiếng Việt tốn 2–3 byte mỗi chữ.
        if s.chars().count() > Self::MAX_CHARS {
            return Err(InvalidValue::new("ServerName", "dài quá 64 ký tự"));
        }
        if s.chars().any(char::is_control) {
            return Err(InvalidValue::new("ServerName", "có ký tự điều khiển"));
        }
        Ok(Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for ServerName {
    type Error = InvalidValue;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<ServerName> for String {
    fn from(name: ServerName) -> Self {
        name.0
    }
}

/// Cổng TCP của server game.
///
/// Không cho cổng dưới 1024: trên Linux cần quyền root, và dễ đụng dịch vụ hệ thống.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct GamePort(u16);

impl GamePort {
    pub const DEFAULT: Self = Self(25565);
    /// Cổng cuối cùng được thử khi tự chọn cổng trống (spec P1, mục 4.4).
    pub const AUTO_LAST: Self = Self(25600);
    const MIN: u16 = 1024;

    pub const fn new(port: u16) -> Result<Self, InvalidValue> {
        if port < Self::MIN {
            return Err(InvalidValue::new("GamePort", "nhỏ hơn 1024"));
        }
        Ok(Self(port))
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

impl fmt::Display for GamePort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl TryFrom<u16> for GamePort {
    type Error = InvalidValue;

    fn try_from(port: u16) -> Result<Self, Self::Error> {
        Self::new(port)
    }
}

impl From<GamePort> for u16 {
    fn from(port: GamePort) -> Self {
        port.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "01JB8Z6Q0K3V5T2N8XWJ4R7M9C";

    #[test]
    fn server_id_parses_and_prints_upper_case() {
        assert_eq!(ServerId::parse(ID).unwrap().to_string(), ID);
        let lower = ServerId::parse(&ID.to_lowercase()).unwrap();
        assert_eq!(lower.to_string(), ID);
    }

    #[test]
    fn server_id_rejects_non_ulid() {
        let bad = [
            "",
            "abc",
            "../../etc",
            "01JB8Z6Q0K3V5T2N8XWJ4R7M9",
            "81JB8Z6Q0K3V5T2N8XWJ4R7M9C",
            "O1JB8Z6Q0K3V5T2N8XWJ4R7M9C",
        ];
        for bad in bad {
            assert!(ServerId::parse(bad).is_err(), "{bad:?}");
        }
        let bad_json = serde_json::from_str::<ServerId>("\"../x\"");
        assert!(bad_json.is_err());
    }

    #[test]
    fn server_id_generate_is_unique_and_round_trips() {
        let a = ServerId::generate();
        let b = ServerId::generate();
        assert_ne!(a, b);
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(serde_json::from_str::<ServerId>(&json).unwrap(), a);
    }

    #[test]
    fn server_name_trims_and_counts_chars() {
        assert_eq!(
            ServerName::parse("  Server của tôi ").unwrap().as_str(),
            "Server của tôi"
        );
        assert!(ServerName::parse(&"ư".repeat(64)).is_ok());
        assert!(ServerName::parse(&"ư".repeat(65)).is_err());
    }

    #[test]
    fn server_name_rejects_empty_and_control_chars() {
        for bad in ["", "   ", "a\nb", "tab\there", "\u{7f}"] {
            assert!(ServerName::parse(bad).is_err(), "{bad:?}");
        }
        assert!(serde_json::from_str::<ServerName>("\"  \"").is_err());
    }

    #[test]
    fn game_port_range() {
        assert_eq!(GamePort::new(25565).unwrap(), GamePort::DEFAULT);
        assert!(GamePort::new(1024).is_ok());
        assert!(GamePort::new(65535).is_ok());
        assert!(GamePort::new(1023).is_err());
        assert!(GamePort::new(0).is_err());
        assert!(serde_json::from_str::<GamePort>("80").is_err());
        assert!(serde_json::from_str::<GamePort>("70000").is_err());
        assert_eq!(
            serde_json::from_str::<GamePort>("25566").unwrap().get(),
            25566
        );
    }
}
