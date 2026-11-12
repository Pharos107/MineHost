//! Giờ trong ngày cho lịch khởi động lại (spec P2, mục 4.1 và 5.7).
//!
//! Chỉ là phép tính trên giờ máy: hàm nhận "giây kể từ nửa đêm" do người gọi đọc từ đồng hồ,
//! để crate này không đụng tới đồng hồ hay múi giờ.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::InvalidValue;

/// Tối đa số mốc khởi động lại mỗi ngày.
pub const MAX_RESTART_TIMES: usize = 4;
const SECONDS_PER_DAY: u32 = 24 * 60 * 60;

/// Một mốc giờ trong ngày, ghi trong `instance.toml` dạng `"HH:MM"` (24 giờ).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DailyTime {
    hour: u8,
    minute: u8,
}

impl DailyTime {
    pub fn new(hour: u8, minute: u8) -> Result<Self, InvalidValue> {
        if hour > 23 || minute > 59 {
            return Err(InvalidValue::new(
                "DailyTime",
                "giờ phải từ 00:00 tới 23:59",
            ));
        }
        Ok(Self { hour, minute })
    }

    /// Chỉ nhận đúng dạng `HH:MM` hai chữ số, như ô chọn giờ của trình duyệt gửi lên.
    pub fn parse(s: &str) -> Result<Self, InvalidValue> {
        let bad = || InvalidValue::new("DailyTime", "giờ phải có dạng HH:MM");
        let (h, m) = s.split_once(':').ok_or_else(bad)?;
        let two_digits = |part: &str| part.len() == 2 && part.bytes().all(|b| b.is_ascii_digit());
        if !two_digits(h) || !two_digits(m) {
            return Err(bad());
        }
        Self::new(h.parse().map_err(|_| bad())?, m.parse().map_err(|_| bad())?)
    }

    pub const fn seconds_of_day(self) -> u32 {
        self.hour as u32 * 3600 + self.minute as u32 * 60
    }
}

impl fmt::Display for DailyTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

impl FromStr for DailyTime {
    type Err = InvalidValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for DailyTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for DailyTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Kiểm tra danh sách mốc người dùng gửi: tối đa [`MAX_RESTART_TIMES`], không trùng. Trả về
/// danh sách đã sắp xếp để `instance.toml` luôn ghi cùng một thứ tự.
pub fn normalize_restart_times(times: &[DailyTime]) -> Result<Vec<DailyTime>, InvalidValue> {
    if times.len() > MAX_RESTART_TIMES {
        return Err(InvalidValue::new("restart_at", "tối đa 4 mốc mỗi ngày"));
    }
    let mut sorted = times.to_vec();
    sorted.sort();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(InvalidValue::new("restart_at", "hai mốc trùng nhau"));
    }
    Ok(sorted)
}

/// Số giây từ `now` (giây kể từ nửa đêm, giờ máy) tới mốc gần nhất, tính cả mốc đúng bằng `now`
/// (trả 0). Không có mốc nào thì `None`.
pub fn seconds_until_next(times: &[DailyTime], now: u32) -> Option<u32> {
    let now = now % SECONDS_PER_DAY;
    times
        .iter()
        .map(|time| {
            let at = time.seconds_of_day();
            if at >= now {
                at - now
            } else {
                at + SECONDS_PER_DAY - now
            }
        })
        .min()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> DailyTime {
        DailyTime::parse(s).unwrap()
    }

    #[test]
    fn parses_only_strict_hh_mm() {
        assert_eq!(t("04:00").to_string(), "04:00");
        assert_eq!(t("23:59").seconds_of_day(), 86_340);
        for bad in [
            "4:00", "24:00", "12:60", "12-00", "12:00:00", "", "ab:cd", "+1:00",
        ] {
            assert!(DailyTime::parse(bad).is_err(), "{bad}");
        }
        let json: DailyTime = serde_json::from_str("\"06:30\"").unwrap();
        assert_eq!(serde_json::to_string(&json).unwrap(), "\"06:30\"");
        assert!(serde_json::from_str::<DailyTime>("\"6:30\"").is_err());
    }

    #[test]
    fn next_occurrence_wraps_to_tomorrow() {
        let times = [t("04:00"), t("16:00")];
        assert_eq!(
            seconds_until_next(&times, t("03:55").seconds_of_day()),
            Some(300)
        );
        assert_eq!(
            seconds_until_next(&times, t("04:00").seconds_of_day()),
            Some(0)
        );
        assert_eq!(
            seconds_until_next(&times, t("04:00").seconds_of_day() + 1),
            Some(43_199)
        );
        assert_eq!(
            seconds_until_next(&times, t("23:00").seconds_of_day()),
            Some(5 * 3600)
        );
        assert_eq!(seconds_until_next(&[], 0), None);
    }

    #[test]
    fn normalize_sorts_and_limits() {
        let sorted = normalize_restart_times(&[t("16:00"), t("04:00")]).unwrap();
        assert_eq!(sorted, [t("04:00"), t("16:00")]);
        assert!(normalize_restart_times(&[t("04:00"), t("04:00")]).is_err());
        let five = ["00:00", "01:00", "02:00", "03:00", "04:00"].map(t);
        assert!(normalize_restart_times(&five).is_err());
        assert!(normalize_restart_times(&[]).unwrap().is_empty());
    }
}
