//! Job chạy nền có tiến độ và hủy được (spec P1, mục 5.7). Chỉ giữ trong RAM (mục 3.1).
//! Danh sách và thời điểm bắt đầu/kết thúc phục vụ trang Tác vụ (spec P2, mục 6.3).

use std::collections::VecDeque;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use mh_core::{ErrorCode, ServerId};
use mh_net::CancellationToken;
use serde::Serialize;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};

use crate::AppError;

/// Số Job đã xong được giữ lại để UI còn xem kết quả.
const KEEP_JOBS: usize = 50;

/// Đủ cho vài Job báo tiến độ byte cùng lúc. Ai bị lỡ tin thì đọc lại [`JobRunner::list`].
const CHANGE_BUFFER: usize = 256;

pub type JobId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    CreateServer,
}

/// Bước đang làm. UI dịch theo `kind`, ví dụ "Tải Java 21 (48/61 MB)".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobStep {
    CheckDisk,
    ResolveVersion { minecraft: String },
    DownloadJava { major: u16 },
    DownloadServer { minecraft: String },
    PrepareFiles,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobProgress {
    pub step: JobStep,
    /// Từ 1.
    pub step_index: u8,
    pub step_count: u8,
    pub bytes_done: Option<u64>,
    pub bytes_total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum JobState {
    Running,
    Succeeded,
    Failed {
        code: ErrorCode,
        params: serde_json::Value,
    },
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JobSnapshot {
    pub id: JobId,
    pub kind: JobKind,
    pub server_id: Option<ServerId>,
    pub state: JobState,
    pub progress: Option<JobProgress>,
    /// Lúc `spawn`, UTC.
    pub started_at: OffsetDateTime,
    /// Lúc sang trạng thái kết thúc. `None` khi Job còn chạy.
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug)]
struct Entry {
    id: JobId,
    status: watch::Sender<JobSnapshot>,
    cancel: CancellationToken,
}

#[derive(Debug)]
pub struct JobRunner {
    next_id: AtomicU64,
    jobs: Mutex<VecDeque<Entry>>,
    downloads: Arc<Semaphore>,
    /// Mọi thay đổi snapshot, kể cả Job vừa tạo, cho WebSocket topic `jobs`.
    changes: broadcast::Sender<JobSnapshot>,
}

/// Thứ mà thân Job dùng để báo tiến độ và biết mình bị hủy.
#[derive(Debug)]
pub struct JobContext {
    status: watch::Sender<JobSnapshot>,
    changes: broadcast::Sender<JobSnapshot>,
    pub cancel: CancellationToken,
    downloads: Arc<Semaphore>,
}

impl JobContext {
    pub fn step(&self, step: JobStep, step_index: u8, step_count: u8) {
        self.status.send_modify(|s| {
            s.progress = Some(JobProgress {
                step,
                step_index,
                step_count,
                bytes_done: None,
                bytes_total: None,
            });
        });
        self.publish();
    }

    pub fn bytes(&self, done: u64, total: Option<u64>) {
        let changed = self.status.send_if_modified(|s| {
            let Some(progress) = s.progress.as_mut() else {
                return false;
            };
            progress.bytes_done = Some(done);
            progress.bytes_total = total;
            true
        });
        if changed {
            self.publish();
        }
    }

    fn publish(&self) {
        broadcast_snapshot(&self.status, &self.changes);
    }

    /// Chờ tới lượt tải (tối đa 2 lượt cùng lúc); hủy được trong lúc chờ.
    pub async fn download_permit(&self) -> Result<OwnedSemaphorePermit, AppError> {
        tokio::select! {
            () = self.cancel.cancelled() => Err(AppError::cancelled()),
            permit = Arc::clone(&self.downloads).acquire_owned() => {
                permit.map_err(|_| AppError::cancelled())
            }
        }
    }

    pub fn check_cancel(&self) -> Result<(), AppError> {
        if self.cancel.is_cancelled() {
            return Err(AppError::cancelled());
        }
        Ok(())
    }

    /// Chạy `fut`, dừng sớm nếu Job bị hủy.
    pub async fn cancellable<T>(
        &self,
        fut: impl Future<Output = Result<T, AppError>>,
    ) -> Result<T, AppError> {
        tokio::select! {
            () = self.cancel.cancelled() => Err(AppError::cancelled()),
            result = fut => result,
        }
    }
}

impl JobRunner {
    pub fn new(max_downloads: usize) -> Self {
        Self {
            next_id: AtomicU64::new(1),
            jobs: Mutex::new(VecDeque::new()),
            downloads: Arc::new(Semaphore::new(max_downloads)),
            changes: broadcast::channel(CHANGE_BUFFER).0,
        }
    }

    pub fn spawn<F, Fut>(&self, kind: JobKind, server_id: Option<ServerId>, body: F) -> JobId
    where
        F: FnOnce(JobContext) -> Fut,
        Fut: Future<Output = Result<(), AppError>> + Send + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let snapshot = running_snapshot(id, kind, server_id);
        let (status, _) = watch::channel(snapshot.clone());
        let cancel = CancellationToken::new();
        self.insert(Entry {
            id,
            status: status.clone(),
            cancel: cancel.clone(),
        });
        // Bản Running đi trước khi thân chạy, để trạng thái kết thúc không vượt Job mới.
        let _ = self.changes.send(snapshot);
        let ctx = JobContext {
            status: status.clone(),
            changes: self.changes.clone(),
            cancel,
            downloads: Arc::clone(&self.downloads),
        };
        watch_outcome(id, status, self.changes.clone(), body(ctx));
        id
    }

    /// Job đang giữ, mới nhất trước. `server_id` có thì chỉ Job của server đó.
    pub fn list(&self, server_id: Option<ServerId>) -> Vec<JobSnapshot> {
        // `push_back` nên cuối hàng là Job tạo sau.
        self.lock()
            .iter()
            .rev()
            .filter(|entry| server_id.is_none_or(|id| entry.status.borrow().server_id == Some(id)))
            .map(|entry| entry.status.borrow().clone())
            .collect()
    }

    /// Mọi thay đổi snapshot kể từ lúc gọi. Không gồm Job đã có sẵn; dùng [`Self::list`].
    pub fn subscribe_all(&self) -> broadcast::Receiver<JobSnapshot> {
        self.changes.subscribe()
    }

    pub fn get(&self, id: JobId) -> Option<JobSnapshot> {
        self.with_entry(id, |e| e.status.borrow().clone())
    }

    pub fn subscribe(&self, id: JobId) -> Option<watch::Receiver<JobSnapshot>> {
        self.with_entry(id, |e| e.status.subscribe())
    }

    pub fn cancel(&self, id: JobId) -> Result<(), AppError> {
        self.with_entry(id, |e| e.cancel.cancel())
            .ok_or_else(|| AppError::not_found(format!("Job {id}")))
    }

    fn insert(&self, entry: Entry) {
        let mut jobs = self.lock();
        jobs.push_back(entry);
        // Bỏ Job đã xong cũ nhất; Job đang chạy thì giữ dù vượt giới hạn.
        while jobs.len() > KEEP_JOBS {
            let finished = jobs
                .iter()
                .position(|e| e.status.borrow().state != JobState::Running);
            match finished {
                Some(i) => {
                    jobs.remove(i);
                }
                None => break,
            }
        }
    }

    fn with_entry<T>(&self, id: JobId, f: impl FnOnce(&Entry) -> T) -> Option<T> {
        self.lock().iter().find(|e| e.id == id).map(f)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, VecDeque<Entry>> {
        // Chỉ bị poison khi một luồng panic lúc đang giữ; dữ liệu bên trong vẫn dùng được.
        self.jobs.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn running_snapshot(id: JobId, kind: JobKind, server_id: Option<ServerId>) -> JobSnapshot {
    JobSnapshot {
        id,
        kind,
        server_id,
        state: JobState::Running,
        progress: None,
        started_at: OffsetDateTime::now_utc(),
        finished_at: None,
    }
}

fn watch_outcome(
    id: JobId,
    status: watch::Sender<JobSnapshot>,
    changes: broadcast::Sender<JobSnapshot>,
    fut: impl Future<Output = Result<(), AppError>> + Send + 'static,
) {
    tokio::spawn(async move {
        let state = outcome(id, fut.await);
        status.send_modify(|snapshot| {
            snapshot.state = state;
            snapshot.finished_at = Some(OffsetDateTime::now_utc());
        });
        broadcast_snapshot(&status, &changes);
    });
}

fn outcome(id: JobId, result: Result<(), AppError>) -> JobState {
    match result {
        Ok(()) => JobState::Succeeded,
        Err(e) if e.code == ErrorCode::JobCancelled => JobState::Cancelled,
        Err(e) => {
            tracing::warn!(job = id, error = %e, "Job thất bại");
            JobState::Failed {
                code: e.code,
                params: e.params,
            }
        }
    }
}

fn broadcast_snapshot(
    status: &watch::Sender<JobSnapshot>,
    changes: &broadcast::Sender<JobSnapshot>,
) {
    // Không có người nghe thì `send` trả lỗi; snapshot vẫn nằm trong `list`.
    let _ = changes.send(status.borrow().clone());
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    async fn finished(jobs: &JobRunner, id: JobId) -> JobSnapshot {
        let mut rx = jobs.subscribe(id).unwrap();
        let wait = rx.wait_for(|s| s.state != JobState::Running);
        tokio::time::timeout(Duration::from_secs(5), wait)
            .await
            .unwrap()
            .unwrap()
            .clone()
    }

    #[tokio::test]
    async fn reports_success_failure_and_cancel() {
        let jobs = JobRunner::new(2);
        let ok = jobs.spawn(JobKind::CreateServer, None, |ctx| async move {
            ctx.step(JobStep::CheckDisk, 1, 1);
            ctx.bytes(5, Some(10));
            Ok(())
        });
        let failed = jobs.spawn(JobKind::CreateServer, None, |_| async {
            Err(AppError::new(ErrorCode::NetworkUnavailable, "x"))
        });
        let cancelled = jobs.spawn(JobKind::CreateServer, None, |ctx| async move {
            ctx.cancel.cancelled().await;
            ctx.check_cancel()
        });
        jobs.cancel(cancelled).unwrap();

        let snap = finished(&jobs, ok).await;
        assert_eq!(snap.state, JobState::Succeeded);
        assert_eq!(snap.progress.unwrap().bytes_done, Some(5));
        let snap = finished(&jobs, failed).await;
        assert!(matches!(
            snap.state,
            JobState::Failed {
                code: ErrorCode::NetworkUnavailable,
                ..
            }
        ));
        assert_eq!(finished(&jobs, cancelled).await.state, JobState::Cancelled);
        assert!(jobs.cancel(999).is_err());
    }

    #[tokio::test]
    async fn keeps_only_last_finished_jobs() {
        let jobs = JobRunner::new(2);
        let ids: Vec<JobId> = (0..KEEP_JOBS + 5)
            .map(|_| jobs.spawn(JobKind::CreateServer, None, |_| async { Ok(()) }))
            .collect();
        for id in &ids[5..] {
            finished(&jobs, *id).await;
        }
        // Thêm một Job nữa để kích hoạt việc dọn.
        let last = jobs.spawn(JobKind::CreateServer, None, |_| async { Ok(()) });
        assert!(jobs.get(last).is_some());
        assert!(jobs.get(ids[0]).is_none());
        assert!(jobs.lock().len() <= KEEP_JOBS);
    }

    #[tokio::test]
    async fn at_most_two_downloads_at_once() {
        let jobs = Arc::new(JobRunner::new(2));
        let running = Arc::new(AtomicU64::new(0));
        let peak = Arc::new(AtomicU64::new(0));
        let ids: Vec<JobId> = (0..5)
            .map(|_| {
                let (running, peak) = (Arc::clone(&running), Arc::clone(&peak));
                jobs.spawn(JobKind::CreateServer, None, move |ctx| async move {
                    let _permit = ctx.download_permit().await?;
                    let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    running.fetch_sub(1, Ordering::SeqCst);
                    Ok(())
                })
            })
            .collect();
        for id in ids {
            finished(&jobs, id).await;
        }
        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn list_is_newest_first_and_records_finish_time() {
        let jobs = JobRunner::new(2);
        let server_a = ServerId::generate();
        let server_b = ServerId::generate();
        let hold = Arc::new(tokio::sync::Notify::new());
        let wait = Arc::clone(&hold);
        let running = jobs.spawn(JobKind::CreateServer, Some(server_a), move |_| async move {
            wait.notified().await;
            Ok(())
        });
        let older = jobs.spawn(JobKind::CreateServer, Some(server_b), |_| async { Ok(()) });
        let newer = jobs.spawn(JobKind::CreateServer, Some(server_a), |_| async { Ok(()) });
        finished(&jobs, older).await;
        finished(&jobs, newer).await;

        let running_snap = jobs.get(running).unwrap();
        assert_eq!(running_snap.state, JobState::Running);
        assert!(running_snap.finished_at.is_none());
        assert_eq!(running_snap.started_at.offset(), time::UtcOffset::UTC);

        let done = jobs.get(newer).unwrap();
        let finished_at = done.finished_at.unwrap();
        assert!(finished_at >= done.started_at);

        let ids: Vec<_> = jobs.list(None).into_iter().map(|snap| snap.id).collect();
        assert_eq!(ids, vec![newer, older, running]);
        let only_a: Vec<_> = jobs
            .list(Some(server_a))
            .into_iter()
            .map(|snap| snap.id)
            .collect();
        assert_eq!(only_a, vec![newer, running]);
        assert_eq!(jobs.list(Some(server_b)).len(), 1);
        assert!(jobs.list(Some(ServerId::generate())).is_empty());

        let mut changes = jobs.subscribe_all();
        let extra = jobs.spawn(JobKind::CreateServer, None, |_| async { Ok(()) });
        let first = changes.try_recv().unwrap();
        assert_eq!(first.id, extra);
        assert_eq!(first.state, JobState::Running);
        assert!(first.finished_at.is_none());
        let update = tokio::time::timeout(Duration::from_secs(5), changes.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(update.id, extra);
        assert!(update.finished_at.is_some());

        hold.notify_one();
        assert!(finished(&jobs, running).await.finished_at.is_some());
    }
}
