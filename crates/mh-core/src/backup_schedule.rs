//! Lịch sao lưu tự động (spec P2, mục 4.1 và 5.7). Chỉ là phép tính trên giờ đưa vào; đọc đồng hồ
//! và múi giờ là việc của `mh-app`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::InvalidValue;
use crate::schedule::DailyTime;

const SECONDS_PER_DAY: i64 = 24 * 60 * 60;
/// `every_<n>h`: n từ 1 tới 168 (một tuần).
pub const MAX_EVERY_HOURS: u16 = 168;
/// Số bản tự động giữ lại: 1..=100, mặc định 10.
pub const DEFAULT_KEEP: u16 = 10;
pub const MAX_KEEP: u16 = 100;

/// Ghi trong `instance.toml` dạng `"off"`, `"every_6h"` hoặc `"daily@04:00"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackupSchedule {
    #[default]
    Off,
    EveryHours(u16),
    DailyAt(DailyTime),
}

impl BackupSchedule {
    pub fn parse(s: &str) -> Result<Self, InvalidValue> {
        let bad = || {
            InvalidValue::new(
                "BackupSchedule",
                "lịch phải là off, every_<n>h hoặc daily@HH:MM",
            )
        };
        if s == "off" {
            return Ok(Self::Off);
        }
        if let Some(time) = s.strip_prefix("daily@") {
            return DailyTime::parse(time).map(Self::DailyAt).map_err(|_| bad());
        }
        let hours = s
            .strip_prefix("every_")
            .and_then(|rest| rest.strip_suffix('h'))
            .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|n| n.parse::<u16>().ok())
            .ok_or_else(bad)?;
        if (1..=MAX_EVERY_HOURS).contains(&hours) {
            Ok(Self::EveryHours(hours))
        } else {
            Err(bad())
        }
    }

    pub const fn is_off(self) -> bool {
        matches!(self, Self::Off)
    }
}

impl fmt::Display for BackupSchedule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => f.write_str("off"),
            Self::EveryHours(n) => write!(f, "every_{n}h"),
            Self::DailyAt(time) => write!(f, "daily@{time}"),
        }
    }
}

impl FromStr for BackupSchedule {
    type Err = InvalidValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for BackupSchedule {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for BackupSchedule {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Số bản `scheduled` giữ lại, 1..=100.
pub fn check_keep(keep: u16) -> Result<u16, InvalidValue> {
    if (1..=MAX_KEEP).contains(&keep) {
        Ok(keep)
    } else {
        Err(InvalidValue::new(
            "keep",
            "số bản giữ lại phải từ 1 tới 100",
        ))
    }
}

/// Lần chạy kế tiếp, giây Unix. `last`: bản `scheduled` gần nhất (giây Unix). `now` và `of_day`
/// (giây kể từ nửa đêm theo giờ máy) là cùng một thời điểm.
///
/// Kết quả nhỏ hơn hoặc bằng `now` nghĩa là đã tới hạn. Chưa có bản nào thì tới hạn ngay.
/// `daily@HH:MM`: tới hạn ở mốc gần nhất đã qua mà chưa có bản nào sau mốc đó, nên controller tắt
/// lúc 04:00 rồi bật lại lúc 09:00 vẫn sao lưu bù một lần.
pub fn next_run(schedule: BackupSchedule, last: Option<i64>, now: i64, of_day: u32) -> Option<i64> {
    match schedule {
        BackupSchedule::Off => None,
        BackupSchedule::EveryHours(hours) => {
            Some(last.map_or(now, |last| last + i64::from(hours) * 3600))
        }
        BackupSchedule::DailyAt(time) => {
            let since =
                (i64::from(of_day) - i64::from(time.seconds_of_day())).rem_euclid(SECONDS_PER_DAY);
            let latest = now - since;
            match last {
                Some(last) if last >= latest => Some(latest + SECONDS_PER_DAY),
                _ => Some(latest),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(raw: &str) -> BackupSchedule {
        BackupSchedule::parse(raw).unwrap()
    }

    #[test]
    fn parses_three_forms_strictly() {
        assert_eq!(s("off"), BackupSchedule::Off);
        assert_eq!(s("every_6h"), BackupSchedule::EveryHours(6));
        assert_eq!(s("every_168h").to_string(), "every_168h");
        assert_eq!(s("daily@04:00").to_string(), "daily@04:00");
        for bad in [
            "",
            "Off",
            "every_0h",
            "every_169h",
            "every_h",
            "every_-1h",
            "every_6",
            "daily@4:00",
            "daily@",
            "hourly",
        ] {
            assert!(BackupSchedule::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(
            serde_json::to_string(&s("every_6h")).unwrap(),
            "\"every_6h\""
        );
        assert!(check_keep(0).is_err() && check_keep(101).is_err());
        assert_eq!(check_keep(100).unwrap(), 100);
    }

    /// Mốc giả: `DAY` là nửa đêm, giây Unix chia hết cho một ngày cho dễ đọc.
    const DAY: i64 = 20_000 * SECONDS_PER_DAY;

    fn at(hour: i64, minute: i64) -> (i64, u32) {
        let secs = hour * 3600 + minute * 60;
        (
            DAY + secs,
            u32::try_from(secs.rem_euclid(SECONDS_PER_DAY)).unwrap(),
        )
    }

    #[test]
    fn every_n_hours_counts_from_last_backup() {
        let (now, of_day) = at(10, 0);
        let six = s("every_6h");
        assert_eq!(next_run(six, None, now, of_day), Some(now));
        assert_eq!(
            next_run(six, Some(now - 3600), now, of_day),
            Some(now + 5 * 3600)
        );
        assert_eq!(
            next_run(six, Some(now - 7 * 3600), now, of_day),
            Some(now - 3600)
        );
        assert_eq!(next_run(s("off"), None, now, of_day), None);
    }

    #[test]
    fn daily_runs_once_per_day_and_catches_up() {
        let daily = s("daily@04:00");
        let (four, _) = at(4, 0);
        // 03:59, đã có bản hôm qua: chờ tới 04:00 hôm nay.
        let (now, of_day) = at(3, 59);
        assert_eq!(
            next_run(daily, Some(four - SECONDS_PER_DAY), now, of_day),
            Some(four)
        );
        // 09:00, chưa có bản nào từ 04:00 hôm nay (controller tắt lúc đó): bù ngay.
        let (now, of_day) = at(9, 0);
        assert_eq!(
            next_run(daily, Some(four - SECONDS_PER_DAY), now, of_day),
            Some(four)
        );
        // 09:00, đã có bản lúc 04:01: lần sau là 04:00 ngày mai.
        assert_eq!(
            next_run(daily, Some(four + 60), now, of_day),
            Some(four + SECONDS_PER_DAY)
        );
        // Qua nửa đêm: 00:30 ngày mới, mốc gần nhất là 04:00 hôm trước.
        let (now, of_day) = at(24, 30);
        assert_eq!(
            next_run(daily, Some(four + 60), now, of_day),
            Some(four + SECONDS_PER_DAY)
        );
        assert_eq!(next_run(daily, None, now, of_day), Some(four));
    }
}
