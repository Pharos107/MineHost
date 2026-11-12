//! Job sao lưu world (spec P2, mục 5.7). Khôi phục nằm ở `restore`; lịch để PR sau.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mh_backup::ArchiveInfo;
use mh_core::{ErrorCode, InstanceSpec, ServerId};
use mh_platform::SafeRoot;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use ulid::Ulid;

use crate::jobs::{JobContext, JobKind, JobStep};
use crate::{App, AppError, JobId, SupervisorHandle};

const SAVE_OFF: &str = "save-off";
const SAVE_ALL: &str = "save-all flush";
const SAVE_ON: &str = "save-on";
/// Paper in đúng cụm này khi `save-all` ghi world xong.
const SAVED_LINE: &str = "Saved the game";
const STEPS_LIVE: u8 = 3;
const STEPS_OFFLINE: u8 = 2;

/// Hạn chờ `Saved the game`. Test truyền hạn ngắn hơn qua [`crate::AppDeps`].
pub(crate) const DEFAULT_SAVE_FLUSH_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    Manual,
    Scheduled,
    BeforeRestore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRequest {
    pub note: Option<String>,
    pub protected: bool,
    pub kind: BackupKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupStarted {
    pub job_id: JobId,
}

/// File mô tả cạnh `.tar.zst` (spec P2, mục 4.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupMeta {
    pub id: String,
    pub server_id: ServerId,
    pub kind: BackupKind,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub size: u64,
    pub sha256: String,
    pub minecraft: String,
    pub loader: mh_core::LoaderKind,
    pub note: Option<String>,
    pub protected: bool,
}

impl App {
    /// Bắt đầu Job sao lưu. Server không tồn tại thì lỗi ngay, không tạo Job.
    pub fn backup(
        self: &Arc<Self>,
        id: ServerId,
        req: BackupRequest,
    ) -> Result<BackupStarted, AppError> {
        self.store.load(id)?;
        let app = Arc::clone(self);
        let job_id = self
            .jobs
            .spawn(JobKind::Backup, Some(id), move |ctx| async move {
                app.run_backup(&ctx, id, req).await
            });
        Ok(BackupStarted { job_id })
    }

    /// Mới nhất trước. File `.json` hỏng bị bỏ qua.
    pub fn list_backups(&self, id: ServerId) -> Result<Vec<BackupMeta>, AppError> {
        self.store.load(id)?;
        let dir = backup_dir(&self.dirs, id);
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        read_backup_list(&dir)
    }

    /// `note` hoặc `protected` là `None` thì giữ nguyên giá trị cũ; `Some(None)` là xóa ghi chú.
    pub fn update_backup(
        &self,
        id: ServerId,
        backup_id: &str,
        note: Option<Option<String>>,
        protected: Option<bool>,
    ) -> Result<BackupMeta, AppError> {
        self.store.load(id)?;
        let path = self.meta_path(id, backup_id)?;
        let mut meta = read_meta(&path)?;
        if let Some(note) = note {
            meta.note = note;
        }
        if let Some(protected) = protected {
            meta.protected = protected;
        }
        write_meta(&path, &meta)?;
        Ok(meta)
    }

    /// Bản `protected` không xóa. Xóa file mô tả trước: còn `.tar.zst` thì lần khởi động sau dọn.
    pub fn delete_backup(&self, id: ServerId, backup_id: &str) -> Result<(), AppError> {
        self.store.load(id)?;
        let parsed = parse_backup_id(backup_id)?;
        let json = self.meta_path(id, backup_id)?;
        let meta = read_meta(&json)?;
        if meta.protected {
            return Err(AppError::new(
                ErrorCode::BackupProtected,
                "bản backup đang được bảo vệ",
            ));
        }
        std::fs::remove_file(&json).map_err(|err| io_at(&json, err))?;
        let dir = backup_dir(&self.dirs, id);
        if let Ok(archive) = backup_file(&dir, &parsed, "tar.zst") {
            remove_archive(&archive);
        }
        Ok(())
    }

    async fn run_backup(
        &self,
        ctx: &JobContext,
        id: ServerId,
        req: BackupRequest,
    ) -> Result<(), AppError> {
        let spec = self.store.load(id)?;
        let handle = self.handle(id)?;
        // Một ảnh trạng thái cho cả Job: server bật giữa chừng thì lần này không gửi lệnh.
        let live = handle.status().state.has_process();
        let steps = if live { STEPS_LIVE } else { STEPS_OFFLINE };
        ctx.step(JobStep::CheckDisk, 1, steps);
        self.ensure_disk(&self.store.paths(id).server_dir).await?;
        ctx.check_cancel()?;

        let paused = if live {
            ctx.step(JobStep::SaveWorld, 2, steps);
            handle.send(SAVE_OFF).await?;
            true
        } else {
            false
        };
        // Gom lỗi vào `result`: `?` ở đây sẽ thoát trước khi kịp gửi `save-on`.
        let result = async {
            if paused {
                self.wait_until_saved(ctx, &handle).await?;
            }
            ctx.check_cancel()?;
            // Bước cuối: 3/3 khi server đang chạy, 2/2 khi tắt.
            ctx.step(JobStep::Compress, steps, steps);
            self.archive_world(ctx, &spec, &req).await
        }
        .await;
        let result = if paused {
            restore_autosave(&handle, id, result).await
        } else {
            result
        };
        // Bản tự động vượt số giữ lại bị xóa, bản tạo tay và bản được bảo vệ không tính (mục 4.2).
        if result.is_ok() && req.kind == BackupKind::Scheduled {
            self.prune_kind(id, BackupKind::Scheduled, usize::from(spec.backup.keep));
        }
        result
    }

    pub(crate) async fn ensure_disk(&self, server_dir: &Path) -> Result<(), AppError> {
        let server_dir = server_dir.to_path_buf();
        let probe = Arc::clone(&self.free_space);
        let backups = self.dirs.backups();
        let (needed, free) = tokio::task::spawn_blocking(move || {
            let needed = dir_size(&server_dir)?;
            Ok::<_, AppError>((needed, probe(&backups)))
        })
        .await
        .map_err(|err| AppError::new(ErrorCode::Internal, format!("đo dung lượng: {err}")))??;
        let Some(free) = free else {
            // Không đọc được ổ thì không chặn: cùng cách với Job tạo server.
            return Ok(());
        };
        if free >= needed {
            return Ok(());
        }
        Err(AppError::new(
            ErrorCode::NotEnoughDisk,
            format!("cần {needed} byte, còn {free}"),
        )
        .with_params(serde_json::json!({
            "available_bytes": free,
            "needed_bytes": needed,
        })))
    }

    /// Nén `server/` và ghi file mô tả. Không đặt bước Job: khôi phục gọi lại hàm này
    /// trong cùng một Job, sau khi đã đặt bước `before_restore`.
    pub(crate) async fn archive_world(
        &self,
        ctx: &JobContext,
        spec: &InstanceSpec,
        req: &BackupRequest,
    ) -> Result<(), AppError> {
        let backup_id = Ulid::generate();
        let dir = backup_dir(&self.dirs, spec.id);
        std::fs::create_dir_all(&dir).map_err(|err| io_at(&dir, err))?;
        let archive = backup_file(&dir, &backup_id, "tar.zst")?;
        let src = self.store.paths(spec.id).server_dir;
        let info = compress_dir(ctx, src, archive.clone()).await?;
        if ctx.cancel.is_cancelled() {
            let _ = std::fs::remove_file(&archive);
            return Err(AppError::cancelled());
        }
        let meta = make_meta(spec, req, &backup_id, &info);
        let json = backup_file(&dir, &backup_id, "json")?;
        if let Err(err) = write_meta(&json, &meta) {
            // Thiếu file mô tả thì bản nén là backup dở. Xóa luôn, không chờ lần khởi động sau.
            let _ = std::fs::remove_file(&archive);
            return Err(err);
        }
        tracing::info!(server = %spec.id, backup = %meta.id, bytes = meta.size, "đã sao lưu");
        Ok(())
    }

    async fn wait_until_saved(
        &self,
        ctx: &JobContext,
        handle: &SupervisorHandle,
    ) -> Result<(), AppError> {
        tokio::select! {
            biased;
            // Hủy phải thắng hạn 60 giây, nếu không Job treo tới lúc `save-all` quá hạn.
            () = ctx.cancel.cancelled() => Err(AppError::cancelled()),
            result = handle.send_and_wait(SAVE_ALL, SAVED_LINE, self.save_flush_timeout) => {
                result.map_err(AppError::from)
            }
        }
    }

    pub(crate) fn meta_path(&self, id: ServerId, backup_id: &str) -> Result<PathBuf, AppError> {
        let parsed = parse_backup_id(backup_id)?;
        let dir = backup_dir(&self.dirs, id);
        if !dir.is_dir() {
            return Err(AppError::not_found(format!("backup {parsed}")));
        }
        let path = backup_file(&dir, &parsed, "json")?;
        if !path.is_file() {
            return Err(AppError::not_found(format!("backup {parsed}")));
        }
        Ok(path)
    }
}

/// `save-on` luôn đi sau `save-off`, kể cả khi nén lỗi hoặc Job bị hủy.
async fn restore_autosave(
    handle: &SupervisorHandle,
    id: ServerId,
    result: Result<(), AppError>,
) -> Result<(), AppError> {
    if let Err(err) = handle.send(SAVE_ON).await {
        tracing::warn!(server = %id, error = %err, "không gửi được save-on sau backup");
        if result.is_ok() {
            return Err(err.into());
        }
    }
    result
}

async fn compress_dir(
    ctx: &JobContext,
    src: PathBuf,
    dest: PathBuf,
) -> Result<ArchiveInfo, AppError> {
    let cancel = ctx.cancel.clone();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::task::spawn_blocking(move || {
        mh_backup::create(
            &src,
            &dest,
            mh_backup::DEFAULT_EXCLUDE,
            |done, total| {
                let _ = tx.send((done, total));
            },
            &cancel,
        )
    });
    tokio::pin!(task);
    loop {
        tokio::select! {
            biased;
            joined = &mut task => {
                apply_progress(ctx, &mut rx);
                return archive_result(joined);
            }
            update = rx.recv() => {
                match update {
                    Some((done, total)) => ctx.bytes(done, Some(total)),
                    None => {
                        let joined = task.await;
                        apply_progress(ctx, &mut rx);
                        return archive_result(joined);
                    }
                }
            }
        }
    }
}

fn apply_progress(ctx: &JobContext, rx: &mut tokio::sync::mpsc::UnboundedReceiver<(u64, u64)>) {
    while let Ok((done, total)) = rx.try_recv() {
        ctx.bytes(done, Some(total));
    }
}

fn archive_result(
    joined: Result<Result<ArchiveInfo, mh_backup::BackupError>, tokio::task::JoinError>,
) -> Result<ArchiveInfo, AppError> {
    match joined {
        Ok(Ok(info)) => Ok(info),
        Ok(Err(err)) => Err(err.into()),
        Err(err) => Err(AppError::new(
            ErrorCode::Internal,
            format!("luồng nén dừng: {err}"),
        )),
    }
}

fn make_meta(
    spec: &InstanceSpec,
    req: &BackupRequest,
    id: &Ulid,
    info: &ArchiveInfo,
) -> BackupMeta {
    let now = OffsetDateTime::now_utc();
    BackupMeta {
        id: id.to_string(),
        server_id: spec.id,
        kind: req.kind,
        created_at: now.replace_nanosecond(0).unwrap_or(now),
        size: info.size,
        sha256: info.sha256.clone(),
        minecraft: spec.game.minecraft.to_string(),
        loader: spec.game.loader,
        note: req.note.clone(),
        protected: req.protected,
    }
}

/// Xóa `.tar.zst.part` và `.tar.zst` không có `.json`. Gọi lúc `App` được tạo và khi reconcile.
pub(crate) fn sweep_incomplete(root: &Path) {
    let Ok(servers) = std::fs::read_dir(root) else {
        return;
    };
    for entry in servers.flatten().filter(|entry| entry.path().is_dir()) {
        sweep_dir(&entry.path());
    }
}

fn sweep_dir(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    let described: HashSet<&str> = names
        .iter()
        .filter_map(|name| name.strip_suffix(".json"))
        .collect();
    for name in &names {
        let stale = name.ends_with(".tar.zst.part")
            || name
                .strip_suffix(".tar.zst")
                .is_some_and(|stem| !described.contains(stem));
        if !stale {
            continue;
        }
        let path = dir.join(name);
        if let Err(err) = std::fs::remove_file(&path) {
            // Xóa hỏng không được chặn khởi động; lần reconcile sau thử lại.
            tracing::warn!(path = %path.display(), error = %err, "không xóa được backup dở");
        }
    }
}

fn read_backup_list(dir: &Path) -> Result<Vec<BackupMeta>, AppError> {
    let mut metas = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|err| io_at(dir, err))? {
        let entry = entry.map_err(|err| io_at(dir, err))?;
        let Some(name) = entry.file_name().into_string().ok() else {
            continue;
        };
        if !name.ends_with(".json") {
            continue;
        }
        match read_meta(&entry.path()) {
            Ok(meta) => metas.push(meta),
            Err(err) => {
                tracing::warn!(path = %entry.path().display(), error = %err, "bỏ qua file mô tả backup hỏng");
            }
        }
    }
    metas.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
    Ok(metas)
}

pub(crate) fn read_meta(path: &Path) -> Result<BackupMeta, AppError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(AppError::not_found(path.display()));
        }
        Err(err) => return Err(io_at(path, err)),
    };
    serde_json::from_str(&text)
        .map_err(|err| AppError::new(ErrorCode::Internal, format!("file mô tả hỏng: {err}")))
}

fn write_meta(path: &Path, meta: &BackupMeta) -> Result<(), AppError> {
    let mut bytes = serde_json::to_vec_pretty(meta).map_err(|err| {
        AppError::new(ErrorCode::Internal, format!("không ghi được mô tả: {err}"))
    })?;
    bytes.push(b'\n');
    mh_platform::fs::write_atomic(path, &bytes)?;
    Ok(())
}

pub(crate) fn backup_dir(dirs: &mh_platform::DataDirs, id: ServerId) -> PathBuf {
    dirs.backups().join(id.to_string())
}

pub(crate) fn backup_file(dir: &Path, id: &Ulid, ext: &str) -> Result<PathBuf, AppError> {
    let root = SafeRoot::new(dir)?;
    Ok(root.join(&format!("{id}.{ext}"))?.into_path_buf())
}

pub(crate) fn parse_backup_id(raw: &str) -> Result<Ulid, AppError> {
    let id = Ulid::from_string(raw).map_err(|_| invalid_backup_id())?;
    // Cùng quy tắc với `ServerId`: không nhận chuỗi Crockford suy ra một ULID khác.
    if !id.to_string().eq_ignore_ascii_case(raw) {
        return Err(invalid_backup_id());
    }
    Ok(id)
}

fn invalid_backup_id() -> AppError {
    AppError::new(ErrorCode::InvalidInput, "mã backup không phải ULID")
}

fn remove_archive(path: &Path) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(path = %path.display(), "file nén của backup đã mất");
        }
        Err(err) => tracing::warn!(path = %path.display(), error = %err, "không xóa được file nén"),
    }
}

fn dir_size(path: &Path) -> Result<u64, AppError> {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|err| io_at(&dir, err))?;
        for entry in entries {
            let entry = entry.map_err(|err| io_at(&dir, err))?;
            let path = entry.path();
            let meta = std::fs::symlink_metadata(&path).map_err(|err| io_at(&path, err))?;
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

fn io_at(path: &Path, err: std::io::Error) -> AppError {
    AppError::new(
        ErrorCode::Internal,
        format!("lỗi I/O tại {}: {err}", path.display()),
    )
}
