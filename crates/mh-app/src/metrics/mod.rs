//! Chỉ số CPU, RAM và dung lượng đĩa của từng server (spec P2 mục 3.5; nâng cấp đa tầng RAM).
//!
//! Mẫu chỉ nằm trong RAM: tắt MineHost là mất. Server không còn process thì không thêm mẫu,
//! bộ đệm cũ được giữ để biểu đồ còn tới lúc controller thoát.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use mh_core::ServerId;
use mh_platform::{ProcessProbe, ProcessUsage, dir_size};
use time::OffsetDateTime;
use tokio::sync::{broadcast, watch};

use crate::{App, AppError, lock};

mod rollup;

use rollup::ServerHistory;
pub use rollup::{MetricSample, MetricsRange};

/// Kết quả `GET /servers/{id}/metrics`.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerMetrics {
    pub samples: Vec<MetricSample>,
    pub disk_bytes: Option<u64>,
    pub running_since: Option<OffsetDateTime>,
    pub oldest_at: Option<OffsetDateTime>,
    pub retention_minutes: u32,
    pub step_seconds: u32,
}

/// Một mẫu vừa được ghi. Không phát lại mẫu cũ — lịch sử đọc qua [`App::metrics`].
#[derive(Debug, Clone, PartialEq)]
pub struct MetricsEvent {
    pub id: ServerId,
    pub sample: MetricSample,
}

/// Chu kỳ task lấy mẫu. Test rút ngắn để không phải chờ 5 giây.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricsConfig {
    pub sample_every: Duration,
    /// Tính lại dung lượng `servers/<id>`.
    pub disk_every: Duration,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            sample_every: Duration::from_secs(5),
            disk_every: Duration::from_secs(5 * 60),
        }
    }
}

#[derive(Debug)]
pub(crate) struct MetricsHub {
    histories: Mutex<HashMap<ServerId, ServerHistory>>,
    disk: Mutex<HashMap<ServerId, u64>>,
    events: broadcast::Sender<MetricsEvent>,
    stop: watch::Sender<bool>,
    started: AtomicBool,
}

struct Measured {
    probe: ProcessProbe,
    usages: Vec<(ServerId, ProcessUsage)>,
    disks: Option<Vec<(ServerId, u64)>>,
}

struct HistorySnapshot {
    samples: Vec<MetricSample>,
    oldest_at: Option<OffsetDateTime>,
    retention_minutes: u32,
    step_seconds: u32,
    disk_bytes: Option<u64>,
}

impl MetricsHub {
    pub(crate) fn new() -> Self {
        Self {
            histories: Mutex::new(HashMap::new()),
            disk: Mutex::new(HashMap::new()),
            events: broadcast::channel(256).0,
            stop: watch::channel(false).0,
            started: AtomicBool::new(false),
        }
    }

    fn subscribe_stop(&self) -> watch::Receiver<bool> {
        self.stop.subscribe()
    }

    fn request_stop(&self) {
        let _ = self.stop.send(true);
    }

    fn clear(&self, id: ServerId) {
        lock(&self.histories).remove(&id);
        lock(&self.disk).remove(&id);
    }

    pub(crate) fn has_history(&self, id: ServerId) -> bool {
        lock(&self.histories).contains_key(&id)
    }

    pub(crate) fn set_retention(&self, id: ServerId, minutes: u32) {
        let mut histories = lock(&self.histories);
        histories
            .entry(id)
            .or_insert_with(|| ServerHistory::new(minutes))
            .set_retention(minutes);
    }

    fn snapshot(
        &self,
        id: ServerId,
        range: MetricsRange,
        configured_retention: u32,
    ) -> HistorySnapshot {
        let (samples, oldest_at, retention_minutes, step_seconds) = {
            let guard = lock(&self.histories);
            match guard.get(&id) {
                Some(history) => {
                    let (samples, step) = history.query(range);
                    (
                        samples,
                        history.oldest_at(),
                        history.retention_minutes(),
                        step,
                    )
                }
                None => (Vec::new(), None, configured_retention, 5),
            }
        };
        HistorySnapshot {
            samples,
            oldest_at,
            retention_minutes,
            step_seconds,
            disk_bytes: lock(&self.disk).get(&id).copied(),
        }
    }

    fn push_all(&self, batch: &[(ServerId, MetricSample, Option<u32>)]) {
        let mut histories = lock(&self.histories);
        for (id, sample, initial_retention) in batch {
            let entry = histories.entry(*id).or_insert_with(|| {
                ServerHistory::new(
                    initial_retention.unwrap_or(mh_core::DEFAULT_METRICS_RETENTION_MINUTES),
                )
            });
            entry.push(sample.clone());
        }
    }

    fn replace_disk(&self, disks: &[(ServerId, u64)]) {
        let mut guard = lock(&self.disk);
        guard.clear();
        guard.extend(disks.iter().copied());
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // `shutdown` là `notify_one`, chỉ đánh thức một người chờ. Task chỉ số có cờ riêng.
        self.metrics_hub.request_stop();
    }
}

impl App {
    /// Mẫu sau `since` (không gồm đúng thời điểm đó), dung lượng thư mục và thời gian chạy.
    pub fn metrics(
        &self,
        id: ServerId,
        since: Option<OffsetDateTime>,
    ) -> Result<ServerMetrics, AppError> {
        self.metrics_range(
            id,
            MetricsRange {
                since,
                until: None,
                max_points: None,
            },
        )
    }

    /// Mẫu trong khoảng `(since, until]`, gộp về tối đa `max_points` điểm nếu có.
    pub fn metrics_range(
        &self,
        id: ServerId,
        range: MetricsRange,
    ) -> Result<ServerMetrics, AppError> {
        let spec = self.store.load(id)?;
        let snap = self
            .metrics_hub
            .snapshot(id, range, spec.metrics.retention_minutes);
        let running_since = lock(&self.supervisors)
            .get(&id)
            .and_then(|handle| handle.status().running_since);
        Ok(ServerMetrics {
            samples: snap.samples,
            disk_bytes: snap.disk_bytes,
            running_since,
            oldest_at: snap.oldest_at,
            retention_minutes: snap.retention_minutes,
            step_seconds: snap.step_seconds,
        })
    }

    pub fn subscribe_metrics(&self) -> broadcast::Receiver<MetricsEvent> {
        self.metrics_hub.events.subscribe()
    }

    /// Chạy task nền. Gọi một lần khi `App` đã nằm trong `Arc`, trong runtime tokio.
    /// Task dừng khi `App` bị hủy.
    pub fn spawn_metrics(self: &Arc<Self>) {
        if self.metrics_hub.started.swap(true, Ordering::SeqCst) {
            return;
        }
        if tokio::runtime::Handle::try_current().is_err() {
            self.metrics_hub.started.store(false, Ordering::SeqCst);
            tracing::warn!("chưa có runtime tokio, chưa lấy mẫu chỉ số");
            return;
        }
        let app = Arc::downgrade(self);
        let mut stop = self.metrics_hub.subscribe_stop();
        let config = self.metrics_config;
        tokio::spawn(async move { run_sampler(app, &mut stop, config).await });
    }

    pub(crate) fn clear_metrics(&self, id: ServerId) {
        self.metrics_hub.clear(id);
    }

    fn record_tick(
        &self,
        usages: Vec<(ServerId, ProcessUsage)>,
        disks: Option<Vec<(ServerId, u64)>>,
    ) {
        let at = OffsetDateTime::now_utc();
        let batch: Vec<_> = usages
            .into_iter()
            .map(|(id, usage)| {
                // Chỉ đọc instance.toml khi server chưa có trong cache; khi đổi cài đặt `update_settings` đã gọi `set_retention`.
                let initial_retention = (!self.metrics_hub.has_history(id)).then(|| {
                    self.store
                        .load(id)
                        .map_or(mh_core::DEFAULT_METRICS_RETENTION_MINUTES, |s| {
                            s.metrics.retention_minutes
                        })
                });
                (
                    id,
                    sample_from(at, usage, self.online_count(id)),
                    initial_retention,
                )
            })
            .collect();
        self.metrics_hub.push_all(&batch);
        if let Some(disks) = &disks {
            self.metrics_hub.replace_disk(disks);
        }
        for (id, sample, _) in batch {
            let _ = self.metrics_hub.events.send(MetricsEvent { id, sample });
        }
    }
}

fn sample_from(at: OffsetDateTime, usage: ProcessUsage, players: u32) -> MetricSample {
    MetricSample {
        at,
        cpu_percent: usage.cpu_percent,
        cpu_max: None,
        memory_bytes: usage.rss_bytes,
        memory_max_bytes: None,
        players: Some(players),
        players_max: None,
    }
}

async fn run_sampler(app: Weak<App>, stop: &mut watch::Receiver<bool>, config: MetricsConfig) {
    let Ok(mut probe) = tokio::task::spawn_blocking(ProcessProbe::new).await else {
        tracing::warn!("không tạo được bộ đo process");
        return;
    };
    let sample_every = config.sample_every.max(Duration::from_millis(1));
    let disk_every = config.disk_every.max(Duration::from_millis(1));
    let mut next_disk = tokio::time::Instant::now();
    loop {
        if *stop.borrow() {
            break;
        }
        let Some(alive) = app.upgrade() else { break };
        let scan_disk = tokio::time::Instant::now() >= next_disk;
        if scan_disk {
            next_disk += disk_every;
        }
        let pids = live_pids(&alive);
        let dirs = scan_disk.then(|| disk_targets(&alive)).flatten();
        drop(alive);

        let Ok(measured) = measure_blocking(probe, pids, dirs).await else {
            break;
        };
        probe = measured.probe;
        let Some(alive) = app.upgrade() else { break };
        alive.record_tick(measured.usages, measured.disks);
        drop(alive);
        if wait_interval(stop, sample_every).await {
            break;
        }
    }
}

fn live_pids(app: &App) -> Vec<(ServerId, u32)> {
    lock(&app.supervisors)
        .iter()
        .filter_map(|(id, handle)| handle.status().pid.map(|pid| (*id, pid)))
        .collect()
}

fn disk_targets(app: &App) -> Option<Vec<(ServerId, PathBuf)>> {
    let listed = match app.store.list() {
        Ok(listed) => listed,
        Err(err) => {
            tracing::warn!(error = %err, "không quét được thư mục server để tính dung lượng");
            return None;
        }
    };
    Some(
        listed
            .into_iter()
            .map(|item| (item.id, app.store.paths(item.id).root))
            .collect(),
    )
}

async fn measure_blocking(
    probe: ProcessProbe,
    pids: Vec<(ServerId, u32)>,
    dirs: Option<Vec<(ServerId, PathBuf)>>,
) -> Result<Measured, ()> {
    tokio::task::spawn_blocking(move || measure(probe, pids, dirs))
        .await
        .map_err(|_| tracing::warn!("task đo chỉ số bị hủy"))
}

fn measure(
    mut probe: ProcessProbe,
    pids: Vec<(ServerId, u32)>,
    dirs: Option<Vec<(ServerId, PathBuf)>>,
) -> Measured {
    let usages = pids
        .into_iter()
        .filter_map(|(id, pid)| probe.usage(pid).map(|usage| (id, usage)))
        .collect();
    let disks = dirs.map(|list| {
        list.into_iter()
            .map(|(id, path)| (id, dir_size(&path)))
            .collect()
    });
    Measured {
        probe,
        usages,
        disks,
    }
}

async fn wait_interval(stop: &mut watch::Receiver<bool>, every: Duration) -> bool {
    tokio::select! {
        biased;
        result = stop.changed() => result.is_err() || *stop.borrow(),
        () = tokio::time::sleep(every) => false,
    }
}
