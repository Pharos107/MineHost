//! Nén world kèm bản chụp `content.toml` và `instance.toml` (spec P3, mục 3.9 và 4.2).
//!
//! Hai file này nằm cạnh `server/` chứ không nằm trong đó, nên backup của P2 không có. Bản chụp đi
//! vào archive dưới `.minehost/`; khôi phục thì lấy ra, trả `content.toml` về chỗ cũ và đặt lại
//! phần `[game]`, `[modpack]` của `instance.toml`. Nhờ vậy khôi phục bản `before_update` đưa server
//! về đúng bản loader, Minecraft và danh sách mod cũ. Tên, cổng, RAM, lịch thì giữ như hiện tại vì
//! người dùng có thể đã sửa sau lúc sao lưu.

use std::path::{Path, PathBuf};

use mh_backup::{ArchiveInfo, Extra};
use mh_core::{ErrorCode, InstanceSpec, ServerId};
use mh_store::ServerPaths;

use crate::jobs::JobContext;
use crate::{App, AppError};

/// Thư mục bản chụp trong archive. World có sẵn thư mục cùng tên thì thư mục đó không được nén.
const SNAPSHOT_DIR: &str = ".minehost";
const CONTENT_ENTRY: &str = ".minehost/content.toml";
const INSTANCE_ENTRY: &str = ".minehost/instance.toml";

/// Bản chụp lấy ra từ archive vừa giải nén.
#[derive(Debug)]
pub(crate) enum Snapshot {
    /// Backup của P2: không có `.minehost/`, giữ nguyên `content.toml` và `instance.toml`.
    Absent,
    Present {
        /// `None` nghĩa là lúc sao lưu server chưa có `content.toml`.
        content: Option<Vec<u8>>,
        instance: Option<String>,
    },
}

/// Nén `server/` vào `dest`, kèm bản chụp hai file cạnh nó.
pub(crate) async fn compress_server(
    ctx: &JobContext,
    paths: &ServerPaths,
    dest: PathBuf,
) -> Result<ArchiveInfo, AppError> {
    let src = paths.server_dir.clone();
    let content = read_optional(&paths.content_file)?;
    let instance = read_optional(&paths.instance_file)?;
    let cancel = ctx.cancel.clone();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::task::spawn_blocking(move || {
        let mut extras = Vec::new();
        if let Some(bytes) = &instance {
            extras.push(Extra {
                path: INSTANCE_ENTRY,
                bytes,
            });
        }
        if let Some(bytes) = &content {
            extras.push(Extra {
                path: CONTENT_ENTRY,
                bytes,
            });
        }
        let mut exclude = mh_backup::DEFAULT_EXCLUDE.to_vec();
        let skip = format!("{SNAPSHOT_DIR}/");
        exclude.push(&skip);
        mh_backup::create_with(
            &src,
            &dest,
            &exclude,
            &extras,
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

/// Lấy bản chụp ra khỏi thư mục vừa giải nén rồi xóa `.minehost/`, để nó không lẫn vào world.
pub(crate) fn take_snapshot(staging: &Path) -> Result<Snapshot, AppError> {
    let dir = staging.join(SNAPSHOT_DIR);
    if !dir.is_dir() {
        return Ok(Snapshot::Absent);
    }
    let content = read_optional(&staging.join(CONTENT_ENTRY))?;
    let instance = read_optional(&staging.join(INSTANCE_ENTRY))?
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    std::fs::remove_dir_all(&dir).map_err(|err| io_at(&dir, err))?;
    Ok(Snapshot::Present { content, instance })
}

impl App {
    /// Gọi sau khi world đã thay xong.
    pub(crate) fn apply_snapshot(&self, id: ServerId, snapshot: Snapshot) -> Result<(), AppError> {
        let Snapshot::Present { content, instance } = snapshot else {
            return Ok(());
        };
        let paths = self.store.paths(id);
        match content {
            Some(bytes) => mh_platform::fs::write_atomic(&paths.content_file, &bytes)?,
            None => match std::fs::remove_file(&paths.content_file) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(io_at(&paths.content_file, err)),
            },
        }
        let Some(text) = instance else {
            return Ok(());
        };
        let old = match InstanceSpec::from_toml(&text) {
            Ok(old) => old,
            Err(err) => {
                // Bản chụp hỏng không làm mất world vừa khôi phục; giữ phiên bản hiện tại.
                tracing::warn!(server = %id, error = %err, "bản chụp instance.toml trong backup hỏng");
                return Ok(());
            }
        };
        let mut spec = self.store.load(id)?;
        spec.game = old.game;
        spec.modpack = old.modpack;
        self.store.save(&spec)?;
        Ok(())
    }
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, AppError> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(io_at(path, err)),
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

fn io_at(path: &Path, err: std::io::Error) -> AppError {
    AppError::new(
        ErrorCode::Internal,
        format!("lỗi I/O tại {}: {err}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;

    #[test]
    fn backup_without_snapshot_dir_is_absent() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("level.dat"), b"x").unwrap();
        assert!(matches!(
            take_snapshot(tmp.path()).unwrap(),
            Snapshot::Absent
        ));
    }

    #[test]
    fn snapshot_is_read_then_removed_from_the_world() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(SNAPSHOT_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("instance.toml"), "schema = 1\n").unwrap();
        let Snapshot::Present { content, instance } = take_snapshot(tmp.path()).unwrap() else {
            panic!("thiếu bản chụp");
        };
        assert_eq!(content, None);
        assert_eq!(instance.as_deref(), Some("schema = 1\n"));
        assert!(!dir.exists());
    }
}
