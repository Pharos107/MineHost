//! Lưu giữ chỉ số đa tầng trong RAM và gộp mẫu theo khung thời gian.
//!
//! Bốn tầng độ mịn: `5s` (tới 1 giờ), `1m` (tới 24 giờ), `5m` (tới 7 ngày), `1h` (tới 9.999 ngày).
//! Mỗi điểm gộp giữ cả trung bình lẫn đỉnh (`*_max`) để không làm phẳng các pha spike ngắn.

use std::collections::VecDeque;

use mh_core::DEFAULT_METRICS_RETENTION_MINUTES;
use time::{Duration as TimeDuration, OffsetDateTime};

const RAW_CAP: usize = 720;
const M1_CAP: usize = 1_440;
const M5_CAP: usize = 2_016;
const H1_CAP: usize = 240_000;
const STEPS_SEC: &[i64] = &[5, 60, 300, 900, 3_600, 14_400, 86_400];

/// Một điểm trên biểu đồ. Các trường `*_max` có mặt khi mẫu được gộp từ nhiều lần đo.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricSample {
    pub at: OffsetDateTime,
    pub cpu_percent: f32,
    pub cpu_max: Option<f32>,
    pub memory_bytes: u64,
    pub memory_max_bytes: Option<u64>,
    pub players: Option<u32>,
    pub players_max: Option<u32>,
}

/// Tham số lọc khoảng thời gian và độ mịn khi đọc lịch sử.
#[derive(Debug, Clone, Copy, Default)]
pub struct MetricsRange {
    pub since: Option<OffsetDateTime>,
    pub until: Option<OffsetDateTime>,
    pub max_points: Option<usize>,
}

#[derive(Debug, Clone)]
struct Bucket {
    at: OffsetDateTime,
    count: u32,
    cpu_sum: f64,
    cpu_max: f32,
    mem_sum: u128,
    mem_max: u64,
    players_sum: u64,
    players_count: u32,
    players_max: u32,
}

impl Bucket {
    fn new(sample: &MetricSample, aligned_at: OffsetDateTime) -> Self {
        let players = sample.players;
        Self {
            at: aligned_at,
            count: 1,
            cpu_sum: f64::from(sample.cpu_percent),
            cpu_max: sample.cpu_max.unwrap_or(sample.cpu_percent),
            mem_sum: u128::from(sample.memory_bytes),
            mem_max: sample.memory_max_bytes.unwrap_or(sample.memory_bytes),
            players_sum: u64::from(players.unwrap_or(0)),
            players_count: u32::from(players.is_some()),
            players_max: sample.players_max.or(players).unwrap_or(0),
        }
    }

    fn fold(&mut self, sample: &MetricSample) {
        self.count = self.count.saturating_add(1);
        self.cpu_sum += f64::from(sample.cpu_percent);
        self.cpu_max = self
            .cpu_max
            .max(sample.cpu_max.unwrap_or(sample.cpu_percent));
        self.mem_sum += u128::from(sample.memory_bytes);
        self.mem_max = self
            .mem_max
            .max(sample.memory_max_bytes.unwrap_or(sample.memory_bytes));
        if let Some(p) = sample.players {
            self.players_sum += u64::from(p);
            self.players_count = self.players_count.saturating_add(1);
            self.players_max = self.players_max.max(sample.players_max.unwrap_or(p));
        }
    }

    fn to_sample(&self) -> MetricSample {
        let count = f64::from(self.count.max(1));
        let mem = (self.mem_sum / u128::from(self.count.max(1))) as u64;
        let players = (self.players_count > 0).then(|| {
            let avg = self.players_sum / u64::from(self.players_count);
            u32::try_from(avg).unwrap_or(u32::MAX)
        });
        MetricSample {
            at: self.at,
            cpu_percent: (self.cpu_sum / count) as f32,
            cpu_max: Some(self.cpu_max),
            memory_bytes: mem,
            memory_max_bytes: Some(self.mem_max),
            players,
            players_max: (self.players_count > 0).then_some(self.players_max),
        }
    }
}

/// Lịch sử chỉ số của một server với 4 tầng lưu giữ trong RAM.
#[derive(Debug)]
pub(crate) struct ServerHistory {
    retention_minutes: u32,
    raw: VecDeque<MetricSample>,
    m1: VecDeque<Bucket>,
    m5: VecDeque<Bucket>,
    h1: VecDeque<Bucket>,
}

impl ServerHistory {
    pub(crate) fn new(retention_minutes: u32) -> Self {
        Self {
            retention_minutes,
            raw: VecDeque::new(),
            m1: VecDeque::new(),
            m5: VecDeque::new(),
            h1: VecDeque::new(),
        }
    }

    pub(crate) fn retention_minutes(&self) -> u32 {
        self.retention_minutes
    }

    pub(crate) fn set_retention(&mut self, minutes: u32) {
        self.retention_minutes = minutes;
        if let Some(latest) = self.latest_at() {
            self.prune(latest);
        }
    }

    pub(crate) fn push(&mut self, sample: MetricSample) {
        let latest = sample.at;
        let cap = raw_cap(self.retention_minutes);
        if self.raw.len() >= cap {
            self.raw.pop_front();
        }
        self.raw.push_back(sample.clone());
        push_bucket(&mut self.m1, &sample, 60, M1_CAP);
        push_bucket(&mut self.m5, &sample, 300, M5_CAP);
        push_bucket(&mut self.h1, &sample, 3_600, H1_CAP);
        self.prune(latest);
    }

    pub(crate) fn oldest_at(&self) -> Option<OffsetDateTime> {
        if self.h1_has_older_than_m5() {
            return self.h1.front().map(|b| b.at);
        }
        if self.m5_has_older_than_m1() {
            return self.m5.front().map(|b| b.at);
        }
        if self.m1_has_older_than_raw() {
            return self.m1.front().map(|b| b.at);
        }
        self.raw.front().map(|s| s.at)
    }

    fn m1_has_older_than_raw(&self) -> bool {
        match (self.m1.front(), self.raw.front()) {
            (Some(b1), Some(r)) => b1.at < align_down(r.at, 60),
            (Some(_), None) => true,
            _ => false,
        }
    }

    fn m5_has_older_than_m1(&self) -> bool {
        match (self.m5.front(), self.m1.front()) {
            (Some(b5), Some(b1)) => b5.at < align_down(b1.at, 300),
            (Some(_), None) => true,
            _ => false,
        }
    }

    fn h1_has_older_than_m5(&self) -> bool {
        match (self.h1.front(), self.m5.front()) {
            (Some(bh), Some(b5)) => bh.at < align_down(b5.at, 3_600),
            (Some(_), None) => true,
            _ => false,
        }
    }

    fn latest_at(&self) -> Option<OffsetDateTime> {
        self.raw.back().map(|s| s.at)
    }

    fn prune(&mut self, latest: OffsetDateTime) {
        let retention = TimeDuration::minutes(i64::from(self.retention_minutes));
        let raw_window = TimeDuration::minutes(i64::from(self.retention_minutes.min(60)));
        let m1_window = retention.min(TimeDuration::hours(24));
        let m5_window = retention.min(TimeDuration::days(7));

        let raw_cap = raw_cap(self.retention_minutes);
        while self.raw.len() > raw_cap {
            self.raw.pop_front();
        }
        // Giữ tối đa `raw_window`, trừ trường hợp test đẩy nhiều mẫu trong vài mili giây.
        prune_samples(&mut self.raw, latest - raw_window);
        prune_buckets(&mut self.m1, latest - m1_window);
        prune_buckets(&mut self.m5, latest - m5_window);
        prune_buckets(&mut self.h1, latest - retention);
    }

    /// Trả các mẫu trong khoảng yêu cầu kèm bước thời gian (giây) giữa các điểm.
    pub(crate) fn query(&self, range: MetricsRange) -> (Vec<MetricSample>, u32) {
        if range.since.is_none() && range.until.is_none() && range.max_points.is_none() {
            return (self.raw.iter().cloned().collect(), 5);
        }
        let Some(latest) = self.latest_at() else {
            return (Vec::new(), 5);
        };
        let oldest = self.oldest_at().unwrap_or(latest);
        let start = range.since.unwrap_or(oldest).max(oldest);
        let end = range.until.unwrap_or(latest).min(latest);
        if end < start {
            return (Vec::new(), 5);
        }
        let span = (end - start).whole_seconds().max(1);
        let limit = i64::try_from(range.max_points.unwrap_or(720).clamp(1, 2_000)).unwrap_or(2_000);

        let raw_covers = self
            .raw
            .front()
            .is_some_and(|s| start >= s.at || !self.m1_has_older_than_raw());
        if raw_covers && span / 5 <= limit {
            let items = self
                .raw
                .iter()
                .filter(|s| in_range(s.at, range.since, range.until))
                .cloned()
                .collect();
            return (items, 5);
        }

        let m1_covers = self
            .m1
            .front()
            .is_some_and(|b| start >= b.at || !self.m5_has_older_than_m1());
        if m1_covers && span / 60 <= limit {
            return (collect_buckets(&self.m1, range.since, range.until), 60);
        }

        let m5_covers = self
            .m5
            .front()
            .is_some_and(|b| start >= b.at || !self.h1_has_older_than_m5());
        if m5_covers && span / 300 <= limit {
            return (collect_buckets(&self.m5, range.since, range.until), 300);
        }

        let target_step = pick_coarse_step(span, limit, m5_covers);
        if target_step == 3_600 {
            return (collect_buckets(&self.h1, range.since, range.until), 3_600);
        }
        let source = if m5_covers { &self.m5 } else { &self.h1 };
        let items: Vec<MetricSample> = collect_buckets(source, range.since, range.until);
        (
            re_bucket(&items, target_step),
            u32::try_from(target_step).unwrap_or(300),
        )
    }
}

impl Default for ServerHistory {
    fn default() -> Self {
        Self::new(DEFAULT_METRICS_RETENTION_MINUTES)
    }
}

fn collect_buckets(
    buf: &VecDeque<Bucket>,
    since: Option<OffsetDateTime>,
    until: Option<OffsetDateTime>,
) -> Vec<MetricSample> {
    buf.iter()
        .filter(|b| in_range(b.at, since, until))
        .map(Bucket::to_sample)
        .collect()
}

fn pick_coarse_step(span: i64, limit: i64, m5_covers: bool) -> i64 {
    let min_step = if m5_covers { 300 } else { 3_600 };
    for &step in STEPS_SEC {
        if step >= min_step && span / step <= limit {
            return step;
        }
    }
    86_400
}

fn raw_cap(retention_minutes: u32) -> usize {
    usize::try_from(retention_minutes.min(60) * 12)
        .unwrap_or(RAW_CAP)
        .clamp(1, RAW_CAP)
}

fn push_bucket(buf: &mut VecDeque<Bucket>, sample: &MetricSample, step_sec: i64, cap: usize) {
    let aligned = align_down(sample.at, step_sec);
    if let Some(last) = buf.back_mut()
        && last.at == aligned
    {
        last.fold(sample);
        return;
    }
    if buf.len() >= cap {
        buf.pop_front();
    }
    buf.push_back(Bucket::new(sample, aligned));
}

fn prune_samples(buf: &mut VecDeque<MetricSample>, cutoff: OffsetDateTime) {
    while buf.front().is_some_and(|s| s.at < cutoff) {
        buf.pop_front();
    }
}

fn prune_buckets(buf: &mut VecDeque<Bucket>, cutoff: OffsetDateTime) {
    while buf.front().is_some_and(|b| b.at < cutoff) {
        buf.pop_front();
    }
}

fn align_down(at: OffsetDateTime, step_sec: i64) -> OffsetDateTime {
    let ts = at.unix_timestamp();
    let aligned = ts - ts.rem_euclid(step_sec);
    OffsetDateTime::from_unix_timestamp(aligned).unwrap_or(at)
}

fn in_range(
    at: OffsetDateTime,
    since: Option<OffsetDateTime>,
    until: Option<OffsetDateTime>,
) -> bool {
    since.is_none_or(|s| at > s) && until.is_none_or(|u| at <= u)
}

fn re_bucket(stitched: &[MetricSample], step_sec: i64) -> Vec<MetricSample> {
    let mut out = Vec::new();
    let mut current: Option<Bucket> = None;
    for sample in stitched {
        let aligned = align_down(sample.at, step_sec);
        match &mut current {
            Some(bucket) if bucket.at == aligned => bucket.fold(sample),
            Some(bucket) => {
                out.push(bucket.to_sample());
                *bucket = Bucket::new(sample, aligned);
            }
            None => current = Some(Bucket::new(sample, aligned)),
        }
    }
    if let Some(bucket) = current {
        out.push(bucket.to_sample());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_at(sec: i64, cpu: f32, mem: u64, players: Option<u32>) -> MetricSample {
        MetricSample {
            at: OffsetDateTime::UNIX_EPOCH + TimeDuration::seconds(sec),
            cpu_percent: cpu,
            cpu_max: None,
            memory_bytes: mem,
            memory_max_bytes: None,
            players,
            players_max: None,
        }
    }

    #[test]
    fn buffer_drops_oldest_after_720() {
        let mut history = ServerHistory::default();
        for i in 0..725 {
            history.push(sample_at(i, 0.0, i as u64, None));
        }
        let (samples, step) = history.query(MetricsRange::default());
        assert_eq!(step, 5);
        assert_eq!(samples.len(), 720);
        assert_eq!(samples.first().map(|s| s.memory_bytes), Some(5));
        assert_eq!(samples.last().map(|s| s.memory_bytes), Some(724));
    }

    #[test]
    fn rollup_preserves_peak_and_supports_long_retention() {
        let mut history = ServerHistory::new(48 * 60);
        // 3 giờ dữ liệu mỗi 5 giây: có 1 spike CPU 99% ở phút thứ 5.
        for i in 0..(3 * 3600 / 5) {
            let sec = i * 5;
            let cpu = if sec == 300 { 99.0 } else { 10.0 };
            history.push(sample_at(sec, cpu, 1024, Some(2)));
        }
        let since = OffsetDateTime::UNIX_EPOCH;
        let until = OffsetDateTime::UNIX_EPOCH + TimeDuration::hours(2);
        let (samples, step) = history.query(MetricsRange {
            since: Some(since),
            until: Some(until),
            max_points: Some(200),
        });
        assert!(step >= 60, "step={step}");
        assert!(samples.len() <= 205, "len={}", samples.len());
        let spike = samples
            .iter()
            .filter_map(|s| s.cpu_max)
            .fold(0.0f32, f32::max);
        assert_eq!(spike, 99.0);
        assert_eq!(history.oldest_at(), Some(OffsetDateTime::UNIX_EPOCH));
    }

    #[test]
    fn lowering_retention_prunes_old_buckets() {
        let mut history = ServerHistory::new(24 * 60);
        for min in 0..180 {
            history.push(sample_at(min * 60, 20.0, 100, None));
        }
        assert_eq!(history.oldest_at(), Some(OffsetDateTime::UNIX_EPOCH));
        history.set_retention(30);
        let expected_cutoff = OffsetDateTime::UNIX_EPOCH + TimeDuration::minutes(179 - 30);
        assert!(history.oldest_at() >= Some(expected_cutoff));
    }
}
