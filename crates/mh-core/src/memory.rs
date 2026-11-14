//! Giới hạn RAM cho JVM và đề xuất RAM theo máy.

use serde::{Deserialize, Serialize};

use crate::error::InvalidValue;

/// `-Xms` và `-Xmx` của server, tính bằng MB. Luôn có `FLOOR_MB <= min <= max <= CEILING_MB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "RawLimits", into = "RawLimits")]
pub struct MemoryLimits {
    min: u32,
    max: u32,
}

#[derive(Serialize, Deserialize)]
struct RawLimits {
    min: u32,
    max: u32,
}

impl MemoryLimits {
    /// Dưới mức này Paper không khởi động nổi.
    pub const FLOOR_MB: u32 = 512;
    /// Chặn số gõ nhầm (thừa vài số 0) trước khi tới JVM.
    pub const CEILING_MB: u32 = 1024 * 1024;

    pub const fn new(min: u32, max: u32) -> Result<Self, InvalidValue> {
        if min < Self::FLOOR_MB {
            return Err(InvalidValue::new("MemoryLimits", "min nhỏ hơn 512 MB"));
        }
        if min > max {
            return Err(InvalidValue::new("MemoryLimits", "min lớn hơn max"));
        }
        if max > Self::CEILING_MB {
            return Err(InvalidValue::new("MemoryLimits", "max lớn quá 1 TB"));
        }
        Ok(Self { min, max })
    }

    pub const fn min_mb(self) -> u32 {
        self.min
    }

    pub const fn max_mb(self) -> u32 {
        self.max
    }
}

impl TryFrom<RawLimits> for MemoryLimits {
    type Error = InvalidValue;

    fn try_from(raw: RawLimits) -> Result<Self, Self::Error> {
        Self::new(raw.min, raw.max)
    }
}

impl From<MemoryLimits> for RawLimits {
    fn from(m: MemoryLimits) -> Self {
        Self {
            min: m.min,
            max: m.max,
        }
    }
}

/// Giới hạn RAM mà OS áp cho cả process server, tính bằng MB (spec P3, mục 3.8).
///
/// JVM dùng thêm bộ nhớ ngoài heap (JIT, metaspace, luồng, bộ đệm mạng): lần chạy dài của P2 đo
/// được Paper nhàn rỗi với `-Xmx512M` chiếm khoảng 1 GB. Công thức chừa rộng tay để chỉ chặn
/// server mất kiểm soát, không chặn server bình thường.
pub const fn memory_limit_mb(max_heap_mb: u32) -> u64 {
    max_heap_mb as u64 * 3 / 2 + 1024
}

/// Đề xuất RAM cho server mới (spec P1, mục 4.3).
///
/// `max` = 1/4 RAM máy, làm tròn xuống bội số 512, kẹp trong 1024..=8192; `min` = `min(1024, max)`.
/// Trên 8 GB Paper ít khi cần thêm cho vài người chơi, còn GC chậm đi khi heap quá lớn.
pub fn suggest_memory_mb(total_mb: u64) -> MemoryLimits {
    suggest_memory_scaled(total_mb, 4, 1024, 8192)
}

/// Đề xuất RAM cho server dựng từ modpack (spec P3, mục 5.7).
///
/// `max` = 1/2 RAM máy, làm tròn xuống bội số 512, kẹp trong 4096..=10240; `min` = `min(1024, max)`.
pub fn suggest_modpack_memory_mb(total_mb: u64) -> MemoryLimits {
    suggest_memory_scaled(total_mb, 2, 4096, 10240)
}

fn suggest_memory_scaled(total_mb: u64, divisor: u64, low: u64, high: u64) -> MemoryLimits {
    const STEP: u64 = 512;
    let scaled = total_mb / divisor / STEP * STEP;
    // Đã kẹp trong `low..=high` (<= 10240) nên ép sang u32 không mất dữ liệu.
    let max = scaled.clamp(low, high) as u32;
    MemoryLimits {
        min: max.min(1024),
        max,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_limit_leaves_room_outside_heap() {
        assert_eq!(memory_limit_mb(512), 1792);
        assert_eq!(memory_limit_mb(6144), 10240);
        assert_eq!(memory_limit_mb(MemoryLimits::CEILING_MB), 1_573_888);
    }

    #[test]
    fn suggestion_for_common_machines() {
        let cases = [(4, 1024), (8, 2048), (16, 4096), (32, 8192), (64, 8192)];
        for (gb, max) in cases {
            let m = suggest_memory_mb(gb * 1024);
            assert_eq!((m.min_mb(), m.max_mb()), (1024, max), "{gb} GB");
        }
    }

    #[test]
    fn modpack_suggestion_for_common_machines() {
        let cases = [(4, 4096), (8, 4096), (16, 8192), (32, 10240), (64, 10240)];
        for (gb, max) in cases {
            let m = suggest_modpack_memory_mb(gb * 1024);
            assert_eq!((m.min_mb(), m.max_mb()), (1024, max), "{gb} GB");
        }
        assert_eq!(suggest_modpack_memory_mb(12 * 1024 - 100).max_mb(), 5632);
        assert_eq!(suggest_modpack_memory_mb(0).max_mb(), 4096);
    }

    #[test]
    fn suggestion_rounds_down_and_clamps_small_machines() {
        // Máy 12 GB thường báo thiếu vài chục MB vì phần cứng giữ lại.
        assert_eq!(suggest_memory_mb(12 * 1024 - 100).max_mb(), 2560);
        assert_eq!(suggest_memory_mb(2048).max_mb(), 1024);
        assert_eq!(suggest_memory_mb(0).max_mb(), 1024);
    }

    #[test]
    fn suggestion_always_passes_validation() {
        for total in (0..200_000).step_by(997) {
            let m = suggest_memory_mb(total);
            assert_eq!(MemoryLimits::new(m.min_mb(), m.max_mb()), Ok(m));
        }
    }

    #[test]
    fn limits_validation() {
        assert!(MemoryLimits::new(1024, 4096).is_ok());
        assert!(MemoryLimits::new(512, 512).is_ok());
        assert!(MemoryLimits::new(511, 4096).is_err());
        assert!(MemoryLimits::new(4096, 1024).is_err());
        assert!(MemoryLimits::new(1024, MemoryLimits::CEILING_MB + 1).is_err());
    }

    #[test]
    fn limits_serde_rejects_invalid() {
        let ok: MemoryLimits = serde_json::from_str(r#"{"min":1024,"max":2048}"#).unwrap();
        assert_eq!(ok.max_mb(), 2048);
        assert!(serde_json::from_str::<MemoryLimits>(r#"{"min":4096,"max":1024}"#).is_err());
    }
}
