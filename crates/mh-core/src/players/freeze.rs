//! Thuộc tính dùng để đóng băng người chơi (spec P2, mục 5.1).

use time::OffsetDateTime;
use time::macros::datetime;

use crate::error::ErrorCode;

/// 1.16, ngày có lệnh `attribute`. Trước mốc này đóng băng không làm được.
const ATTRIBUTE_SINCE: OffsetDateTime = datetime!(2020-06-23 0:00 UTC);
/// 1.20.5 thêm `jump_strength`.
const JUMP_STRENGTH_SINCE: OffsetDateTime = datetime!(2024-04-23 0:00 UTC);
/// 1.21.2 bỏ tiền tố `generic.`.
const BARE_NAMES_SINCE: OffsetDateTime = datetime!(2024-10-22 0:00 UTC);

/// Một thuộc tính `attribute`. `default_base` là giá trị của người chơi mới, dùng khi bỏ đóng băng.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreezeAttribute {
    /// Tên trong lệnh, ví dụ `generic.movement_speed` hoặc `movement_speed`.
    pub name: &'static str,
    /// Giá trị `base` mặc định, viết sẵn để lệnh không phụ thuộc cách in số thực.
    pub default_base: &'static str,
}

/// Bản Minecraft phát hành trước 1.16, chưa có lệnh `attribute`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("phiên bản Minecraft chưa có lệnh attribute")]
pub struct UnsupportedVersion;

impl UnsupportedVersion {
    pub const fn code(self) -> ErrorCode {
        ErrorCode::UnsupportedByVersion
    }
}

/// Thuộc tính cần đặt về 0 để đóng băng, kèm giá trị để bỏ đóng băng.
///
/// So theo instant UTC của `releaseTime`, không so chuỗi phiên bản: Mojang đã đổi cách đánh số.
/// Mốc là 00:00 UTC đúng ngày ra mắt. Manifest Mojang ghi giờ UTC, nên một bản ra trong ngày đó
/// đứng sau mốc.
pub fn freeze_attributes(
    release_time: OffsetDateTime,
) -> Result<Vec<FreezeAttribute>, UnsupportedVersion> {
    if release_time < ATTRIBUTE_SINCE {
        return Err(UnsupportedVersion);
    }
    let generic_prefix = release_time < BARE_NAMES_SINCE;
    let mut attrs = Vec::with_capacity(2);
    attrs.push(FreezeAttribute {
        name: movement_name(generic_prefix),
        default_base: "0.1",
    });
    if release_time >= JUMP_STRENGTH_SINCE {
        attrs.push(FreezeAttribute {
            name: jump_name(generic_prefix),
            default_base: "0.42",
        });
    }
    Ok(attrs)
}

fn movement_name(generic_prefix: bool) -> &'static str {
    if generic_prefix {
        "generic.movement_speed"
    } else {
        "movement_speed"
    }
}

fn jump_name(generic_prefix: bool) -> &'static str {
    if generic_prefix {
        "generic.jump_strength"
    } else {
        "jump_strength"
    }
}
