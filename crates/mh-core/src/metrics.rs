//! Giới hạn cấu hình lưu chỉ số CPU, RAM và người chơi cho từng server.

use crate::error::InvalidValue;

/// Mặc định giữ 1 giờ lịch sử trong RAM.
pub const DEFAULT_METRICS_RETENTION_MINUTES: u32 = 60;
/// Tối thiểu 5 phút.
pub const MIN_METRICS_RETENTION_MINUTES: u32 = 5;
/// Tối đa 9.999 ngày (tính bằng phút).
pub const MAX_METRICS_RETENTION_MINUTES: u32 = 9_999 * 24 * 60;

/// Kiểm tra số phút giữ lịch sử chỉ số nằm trong `5..=14_398_560`.
pub fn check_metrics_retention(minutes: u32) -> Result<u32, InvalidValue> {
    if !(MIN_METRICS_RETENTION_MINUTES..=MAX_METRICS_RETENTION_MINUTES).contains(&minutes) {
        return Err(InvalidValue::new(
            "metrics_retention_minutes",
            "phải từ 5 phút tới 9.999 ngày",
        ));
    }
    Ok(minutes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_bounds() {
        assert_eq!(check_metrics_retention(5), Ok(5));
        assert_eq!(check_metrics_retention(60), Ok(60));
        assert_eq!(
            check_metrics_retention(MAX_METRICS_RETENTION_MINUTES),
            Ok(MAX_METRICS_RETENTION_MINUTES)
        );
        assert!(check_metrics_retention(4).is_err());
        assert!(check_metrics_retention(MAX_METRICS_RETENTION_MINUTES + 1).is_err());
    }
}
