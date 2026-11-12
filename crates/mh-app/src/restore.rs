//! Job khôi phục world (spec P2, mục 5.7). Server đang chạy thì tắt trước, rồi mới thay thư mục.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use mh_core::{ErrorCode, InstanceSpec, ServerId};
use ulid::Ulid;

use crate::backup::{
    BackupKind, BackupMeta, BackupRequest, backup_dir, backup_file, parse_backup_id, read_meta,
};
use crate::jobs::{JobContext, JobKind, JobStep};
use crate::{App, AppError, JobId, SupervisorHandle};

/// Số bản `before_restore` không bảo vệ được giữ. Bản cũ hơn bị xóa sau khi khôi phục xong.
const KEEP_BEFORE_RESTORE: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestoreStarted {
    pub job_id: JobId,
}

impl App {
    /// Bắt đầu Job khôi phục. Thiếu server hoặc file mô tả thì lỗi ngay, không tạo Job.
    pub fn restore(
        self: &Arc<Self>,
        id: ServerId,
        backup_id: &str,
    ) -> Result<RestoreStarted, AppError> {
        self.store.load(id)?;
        let (meta, archive) = self.load_archive(id, backup_id)?;
        let app = Arc::clone(self);
        let job_id = self
            .jobs
            .spawn(JobKind::Restore, Some(id), move |ctx| async move {
                app.run_restore(&ctx, id, meta, archive).await
            });
        Ok(RestoreStarted { job_id })
    }

    fn load_archive(
        &self,
        id: ServerId,
        backup_id: &str,
    ) -> Result<(BackupMeta, PathBuf), AppError> {
        let json = self.meta_path(id, backup_id)?;
        let meta = read_meta(&json)?;
        let parsed = parse_backup_id(backup_id)?;
        let archive = backup_file(&backup_dir(&self.dirs, id), &parsed, "tar.zst")?;
        if !archive.is_file() {
            return Err(AppError::not_found(format!("backup {parsed}")));
        }
        Ok((meta, archive))
    }

    async fn run_restore(
        &self,
        ctx: &JobContext,
        id: ServerId,
        meta: BackupMeta,
        archive: PathBuf,
    ) -> Result<(), AppError> {
        let backup_label = meta.id.clone();
        let spec = self.store.load(id)?;
        let handle = self.handle(id)?;
        let live = server_is_live(&handle);
        let steps = if live { 4 } else { 3 };
        let mut index = 1u8;
        if live {
            ctx.step(JobStep::StopServer, index, steps);
            self.stop_for_restore(&handle).await?;
            index += 1;
        }
        ctx.check_cancel()?;
        ctx.step(JobStep::BeforeRestore, index, steps);
        index += 1;
        self.capture_before_restore(ctx, &spec).await?;
        ctx.check_cancel()?;
        let paths = self.store.paths(id);
        let token = Ulid::generate();
        let staging = paths.root.join(format!(".restore-{token}"));
        let old = paths.root.join(format!(".old-{token}"));
        ctx.step(JobStep::Unpack, index, steps);
        index += 1;
        self.unpack_into(ctx, &archive, &meta.sha256, &staging)
            .await?;
        if let Err(err) = ready_to_swap(ctx, &handle) {
            return Err(discard(&staging, err));
        }
        ctx.step(JobStep::Swap, index, steps);
        swap_world(&paths.server_dir, &staging, &old)?;
        self.prune_kind(id, BackupKind::BeforeRestore, KEEP_BEFORE_RESTORE);
        self.notify_changed(id);
        tracing::info!(server = %id, backup = %backup_label, "đã khôi phục");
        Ok(())
    }

    async fn stop_for_restore(&self, handle: &SupervisorHandle) -> Result<(), AppError> {
        // Hủy Job không cắt bước này: bỏ dở sẽ để server nằm ở Stopping.
        let status = handle.stop_and_wait(self.shutdown_timeout()).await;
        if status.state.has_process() || status.restart_pending {
            return Err(AppError::new(
                ErrorCode::ServerBusy,
                "server không dừng trước khi khôi phục",
            ));
        }
        Ok(())
    }

    async fn capture_before_restore(
        &self,
        ctx: &JobContext,
        spec: &InstanceSpec,
    ) -> Result<(), AppError> {
        self.ensure_disk(&self.store.paths(spec.id).server_dir)
            .await?;
        ctx.check_cancel()?;
        let req = BackupRequest {
            note: None,
            protected: false,
            kind: BackupKind::BeforeRestore,
        };
        // Server đã tắt: nén thẳng, không gửi save-off / save-all / save-on.
        self.archive_world(ctx, spec, &req).await
    }

    async fn unpack_into(
        &self,
        ctx: &JobContext,
        archive: &Path,
        sha256: &str,
        dest: &Path,
    ) -> Result<(), AppError> {
        let archive = archive.to_path_buf();
        let sha256 = sha256.to_owned();
        let dest_buf = dest.to_path_buf();
        let cancel = ctx.cancel.clone();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let task = tokio::task::spawn_blocking(move || {
            mh_backup::restore(
                &archive,
                &sha256,
                &dest_buf,
                |done, total| {
                    let _ = tx.send((done, total));
                },
                &cancel,
            )
        });
        let result = drive_unpack(ctx, &mut rx, task).await;
        if result.is_err() {
            // Sai hash hoặc giải nén dở: chưa đổi tên `server/`, xóa thư mục tạm.
            remove_tree(dest);
        }
        result
    }

    /// Xóa bản `kind` không bảo vệ vượt `keep`. Lỗi xóa không làm hỏng Job vừa xong.
    pub(crate) fn prune_kind(&self, id: ServerId, kind: BackupKind, keep: usize) {
        let list = match self.list_backups(id) {
            Ok(list) => list,
            Err(err) => {
                tracing::warn!(server = %id, error = %err, "không liệt kê được backup để dọn");
                return;
            }
        };
        for backup_id in stale_backup_ids(&list, kind, keep) {
            if let Err(err) = self.delete_backup(id, &backup_id) {
                tracing::warn!(
                    server = %id,
                    backup = %backup_id,
                    error = %err,
                    "không xóa được backup cũ"
                );
            }
        }
    }
}

/// `metas` mới nhất trước, đúng thứ tự `list_backups`. Bản `protected` không chiếm suất `keep`.
pub(crate) fn stale_backup_ids(metas: &[BackupMeta], kind: BackupKind, keep: usize) -> Vec<String> {
    metas
        .iter()
        .filter(|meta| meta.kind == kind && !meta.protected)
        .skip(keep)
        .map(|meta| meta.id.clone())
        .collect()
}

fn server_is_live(handle: &SupervisorHandle) -> bool {
    let status = handle.status();
    status.state.has_process() || status.restart_pending
}

fn ready_to_swap(ctx: &JobContext, handle: &SupervisorHandle) -> Result<(), AppError> {
    ctx.check_cancel()?;
    // Có thể có người bấm Bật trong lúc giải nén. Đổi tên lúc process còn sống sẽ hỏng world.
    if server_is_live(handle) {
        return Err(AppError::new(
            ErrorCode::ServerBusy,
            "server bật lại trước khi thay world",
        ));
    }
    Ok(())
}

fn discard(path: &Path, err: AppError) -> AppError {
    remove_tree(path);
    err
}

/// Đổi tên sau cùng. Lỗi trước lần đổi đầu thì xóa bản bung; lỗi giữa hai lần thì trả `.old` về `server`.
fn swap_world(server: &Path, staging: &Path, old: &Path) -> Result<(), AppError> {
    if !server.exists() {
        // World trên đĩa đã mất sau lúc sao lưu hiện trạng. Bản vừa bung là bản người dùng chọn.
        return std::fs::rename(staging, server).map_err(|err| io_at(staging, err));
    }
    if let Err(err) = std::fs::rename(server, old) {
        remove_tree(staging);
        return Err(io_at(server, err));
    }
    if let Err(err) = std::fs::rename(staging, server) {
        return rollback_swap(server, staging, old, err);
    }
    if let Err(err) = std::fs::remove_dir_all(old) {
        // `server/` đã là world mới. Lần khởi động sau xóa `.old-*` còn sót.
        tracing::warn!(path = %old.display(), error = %err, "không xóa được world cũ sau khôi phục");
    }
    Ok(())
}

fn rollback_swap(
    server: &Path,
    staging: &Path,
    old: &Path,
    err: std::io::Error,
) -> Result<(), AppError> {
    if let Err(back) = std::fs::rename(old, server) {
        tracing::error!(
            server = %server.display(),
            old = %old.display(),
            error = %back,
            "không đổi lại được world sau khi thay thất bại"
        );
        return Err(io_at(old, back));
    }
    remove_tree(staging);
    Err(io_at(server, err))
}

async fn drive_unpack(
    ctx: &JobContext,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<(u64, u64)>,
    task: tokio::task::JoinHandle<Result<(), mh_backup::BackupError>>,
) -> Result<(), AppError> {
    tokio::pin!(task);
    loop {
        tokio::select! {
            biased;
            joined = &mut task => return unpack_result(ctx, rx, joined),
            update = rx.recv() => {
                if let Some((done, total)) = update {
                    ctx.bytes(done, positive_total(total));
                } else {
                    let joined = task.await;
                    return unpack_result(ctx, rx, joined);
                }
            }
        }
    }
}

fn unpack_result(
    ctx: &JobContext,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<(u64, u64)>,
    joined: Result<Result<(), mh_backup::BackupError>, tokio::task::JoinError>,
) -> Result<(), AppError> {
    while let Ok((done, total)) = rx.try_recv() {
        ctx.bytes(done, positive_total(total));
    }
    match joined {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => Err(err.into()),
        Err(err) => Err(AppError::new(
            ErrorCode::Internal,
            format!("luồng giải nén dừng: {err}"),
        )),
    }
}

fn positive_total(total: u64) -> Option<u64> {
    // Lúc bung file, mh-backup báo tổng 0 vì chưa biết kích thước world.
    (total > 0).then_some(total)
}

fn remove_tree(path: &Path) {
    match std::fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "không xóa được thư mục khôi phục dở");
        }
    }
}

fn io_at(path: &Path, err: std::io::Error) -> AppError {
    AppError::new(
        ErrorCode::Internal,
        format!("lỗi I/O tại {}: {err}", path.display()),
    )
}

/// Dọn `.restore-*` và `.old-*` sót sau mất điện. Gọi lúc tạo `App` và khi reconcile,
/// trước khi bật server.
///
/// `server/` không còn mà `.old-*` còn: world nằm ở bản `.old-*` mới nhất (ULID tăng theo thời gian),
/// đổi nó về `server/`. `server/` còn thì world hiện tại thắng, chỉ xóa thư mục tạm.
pub(crate) fn sweep_interrupted(servers_root: &Path) {
    let Ok(entries) = std::fs::read_dir(servers_root) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            sweep_one(&entry.path());
        }
    }
}

fn sweep_one(root: &Path) {
    let (mut olds, restores) = staged_dirs(root);
    let server = root.join("server");
    if !server.is_dir() {
        olds.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        if let Some(newest) = olds.pop()
            && let Err(err) = std::fs::rename(&newest, &server)
        {
            // Để nguyên thư mục trên đĩa. Lần khởi động sau thử đổi tên lại.
            tracing::warn!(path = %newest.display(), error = %err, "không trả .old-* về server/");
        }
    }
    for path in olds.into_iter().chain(restores) {
        remove_tree(&path);
    }
}

fn staged_dirs(root: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut olds = Vec::new();
    let mut restores = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return (olds, restores);
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name.starts_with(".old-") {
            olds.push(entry.path());
        } else if name.starts_with(".restore-") {
            restores.push(entry.path());
        }
    }
    (olds, restores)
}

#[cfg(test)]
mod tests {
    use super::{BackupKind, BackupMeta, stale_backup_ids};
    use mh_core::{LoaderKind, ServerId};
    use time::OffsetDateTime;

    fn meta(id: &str, kind: BackupKind, protected: bool) -> BackupMeta {
        BackupMeta {
            id: id.to_owned(),
            server_id: ServerId::generate(),
            kind,
            created_at: OffsetDateTime::UNIX_EPOCH,
            size: 1,
            sha256: "ab".to_owned(),
            minecraft: "1.21.4".to_owned(),
            loader: LoaderKind::Paper,
            note: None,
            protected,
        }
    }

    #[test]
    fn keeps_three_newest_unprotected_before_restore() {
        let metas = vec![
            meta("p", BackupKind::BeforeRestore, true),
            meta("a", BackupKind::BeforeRestore, false),
            meta("b", BackupKind::BeforeRestore, false),
            meta("c", BackupKind::BeforeRestore, false),
            meta("m", BackupKind::Manual, false),
            meta("d", BackupKind::BeforeRestore, false),
        ];
        assert_eq!(
            stale_backup_ids(&metas, BackupKind::BeforeRestore, 3),
            vec!["d".to_owned()]
        );
    }
}
