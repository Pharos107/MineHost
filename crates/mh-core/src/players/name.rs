//! Tên người chơi và UUID offline.

use std::fmt;
use std::str::FromStr;

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

use crate::error::InvalidValue;

/// Tên tài khoản Java: `[A-Za-z0-9_]{3,16}`.
///
/// Chỉ tạo qua [`PlayerName::parse`]. Lệnh console ghép tên thẳng vào dòng stdin,
/// nên một tên chưa kiểm tra có thể chèn thêm lệnh.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PlayerName(String);

impl PlayerName {
    pub fn parse(name: &str) -> Result<Self, InvalidValue> {
        let ok_len = (3..=16).contains(&name.len());
        // Bộ ký tự là ASCII, nên đếm byte là đếm ký tự. Byte không phải ASCII bị từ chối.
        let ok_chars = name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
        if ok_len && ok_chars {
            Ok(Self(name.to_owned()))
        } else {
            Err(InvalidValue::new(
                "PlayerName",
                "phải khớp [A-Za-z0-9_]{3,16}",
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PlayerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for PlayerName {
    type Err = InvalidValue;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::parse(name)
    }
}

impl TryFrom<String> for PlayerName {
    type Error = InvalidValue;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        Self::parse(&name)
    }
}

impl From<PlayerName> for String {
    fn from(name: PlayerName) -> Self {
        name.0
    }
}

/// UUID offline như Minecraft tính: MD5 của `OfflinePlayer:` + tên, dạng có gạch ngang.
///
/// Không phải hàm băm để bảo mật. Minecraft gọi `UUID.nameUUIDFromBytes` với đúng tiền tố
/// này; đổi thuật toán là lệch UUID với world đã chơi ở chế độ offline.
pub fn offline_uuid(name: &PlayerName) -> String {
    let mut hasher = Md5::new();
    hasher.update(b"OfflinePlayer:");
    hasher.update(name.as_str().as_bytes());
    let digest = hasher.finalize();
    // `as_ref` ra đúng 16 byte của MD5. Gán kiểu để vòng lặp không phải đoán.
    let raw: &[u8] = digest.as_ref();
    let mut bytes = [0u8; 16];
    for (slot, byte) in bytes.iter_mut().zip(raw) {
        *slot = *byte;
    }
    // Cùng bit mà `UUID.nameUUIDFromBytes` đặt: version 3, variant IETF.
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format_uuid(&bytes)
}

fn format_uuid(bytes: &[u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        out.push(char::from(HEX[usize::from(*byte >> 4)]));
        out.push(char::from(HEX[usize::from(*byte & 0x0f)]));
    }
    out
}
