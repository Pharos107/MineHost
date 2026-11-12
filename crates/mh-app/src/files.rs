//! Dịch vụ tệp của một server (spec P2, mục 3.6 và 5.7).
//!
//! Gốc là `servers/<id>/server/`. Mọi đường dẫn người dùng đi qua `SafeRoot::join`.
//! Thao tác đĩa chạy trong `spawn_blocking`, trừ `save_upload`: stream multipart của PR-11
//! là `AsyncRead`, và `tokio::fs` tự đưa từng lần ghi vào `spawn_blocking`.

mod disk;

use std::path::{Path, PathBuf};

use mh_core::{ErrorCode, ServerId};
use mh_platform::PlatformError;
use time::OffsetDateTime;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

use crate::{App, AppError};

/// Ngưỡng trình sửa văn bản (spec P2, mục 4.5). Giới hạn tải lên do route truyền vào.
pub const TEXT_LIMIT: u64 = 1024 * 1024;

const SERVER_JAR: &str = "server.jar";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    File,
    Dir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub name: String,
    pub kind: FileKind,
    pub size: Option<u64>,
    pub modified: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextFile {
    pub text: String,
    pub etag: String,
    pub size: u64,
}

impl App {
    pub async fn list_dir(&self, id: ServerId, path: &str) -> Result<Vec<FileEntry>, AppError> {
        let (store, path) = (self.store.clone(), path.to_owned());
        blocking(move || disk::list_dir_sync(&disk::open_root(&store, id)?, &path)).await
    }

    pub async fn read_text(&self, id: ServerId, path: &str) -> Result<TextFile, AppError> {
        let (store, path) = (self.store.clone(), path.to_owned());
        blocking(move || disk::read_text_sync(&disk::open_root(&store, id)?, &path)).await
    }

    /// Mở một file để route tải xuống stream. Thư mục bị từ chối, không đọc sẵn vào RAM.
    pub async fn open_download(
        &self,
        id: ServerId,
        path: &str,
    ) -> Result<(String, tokio::fs::File), AppError> {
        let (store, path) = (self.store.clone(), path.to_owned());
        let (name, file) = blocking(move || disk::open_download_sync(&store, id, &path)).await?;
        Ok((name, tokio::fs::File::from_std(file)))
    }

    pub async fn write_text(
        &self,
        id: ServerId,
        path: &str,
        text: &str,
        if_match: Option<&str>,
    ) -> Result<String, AppError> {
        self.guard_jar(id, path)?;
        let (store, path, text, if_match) = (
            self.store.clone(),
            path.to_owned(),
            text.to_owned(),
            if_match.map(str::to_owned),
        );
        blocking(move || {
            disk::write_text_sync(
                &disk::open_root(&store, id)?,
                &path,
                &text,
                if_match.as_deref(),
            )
        })
        .await
    }

    pub async fn create_dir(&self, id: ServerId, path: &str) -> Result<(), AppError> {
        self.guard_jar(id, path)?;
        let (store, path) = (self.store.clone(), path.to_owned());
        blocking(move || disk::create_dir_sync(&disk::open_root(&store, id)?, &path)).await
    }

    pub async fn rename(&self, id: ServerId, from: &str, to: &str) -> Result<(), AppError> {
        self.guard_jar(id, from)?;
        self.guard_jar(id, to)?;
        let (store, from, to) = (self.store.clone(), from.to_owned(), to.to_owned());
        blocking(move || disk::rename_sync(&disk::open_root(&store, id)?, &from, &to)).await
    }

    /// Chuyển vào `trash/<id>/files/<YYYYMMDD-HHMMSS>/<đường dẫn cũ>` và trả đường dẫn đó.
    pub async fn delete(&self, id: ServerId, path: &str) -> Result<PathBuf, AppError> {
        self.guard_jar(id, path)?;
        let (store, trash, path) = (self.store.clone(), self.dirs.trash(), path.to_owned());
        blocking(move || disk::delete_sync(&store, &trash, id, &path)).await
    }

    /// Ghi upload vào `<đích>.part` rồi `rename`. Quá `max_bytes` thì xóa `.part`.
    ///
    /// `AsyncRead` khớp field multipart của axum (PR-11): không gom 512 MiB vào RAM,
    /// và không có `std::io::Read` sẵn trên stream đó.
    pub async fn save_upload(
        &self,
        id: ServerId,
        dir: &str,
        file_name: &str,
        reader: impl AsyncRead + Send + Unpin,
        max_bytes: u64,
    ) -> Result<(), AppError> {
        validate_file_name(file_name)?;
        self.guard_jar(id, &child_path(dir, file_name))?;
        let (store, dir, file_name) = (self.store.clone(), dir.to_owned(), file_name.to_owned());
        let (dest, part) =
            blocking(move || disk::upload_paths(&store, id, &dir, &file_name)).await?;
        stream_upload(dest, part, reader, max_bytes).await
    }

    /// `server.jar` đúng ở gốc đang bị process Java mở. Không tạo supervisor chỉ để hỏi.
    fn guard_jar(&self, id: ServerId, user_path: &str) -> Result<(), AppError> {
        if !is_root_server_jar(user_path) {
            return Ok(());
        }
        let busy = crate::lock(&self.supervisors)
            .get(&id)
            .is_some_and(|handle| handle.status().state.has_process());
        if busy { Err(busy_jar()) } else { Ok(()) }
    }
}

async fn stream_upload(
    dest: PathBuf,
    part: PathBuf,
    mut reader: impl AsyncRead + Unpin,
    max_bytes: u64,
) -> Result<(), AppError> {
    // `guard` khai báo trước `file` nên khi lỗi file đóng trước, rồi `.part` mới bị xóa.
    let mut guard = PartGuard {
        path: part,
        committed: false,
    };
    let mut file = tokio::fs::File::create(&guard.path)
        .await
        .map_err(|err| io_err(&guard.path, err))?;
    copy_limited(&mut file, &mut reader, max_bytes, &guard.path).await?;
    file.sync_all()
        .await
        .map_err(|err| io_err(&guard.path, err))?;
    drop(file);
    // `rename` trên Unix ghi đè. Kiểm tra lại để không nuốt file vừa xuất hiện.
    if tokio::fs::try_exists(&dest)
        .await
        .map_err(|err| io_err(&dest, err))?
    {
        return Err(exists(&dest.display().to_string()));
    }
    tokio::fs::rename(&guard.path, &dest)
        .await
        .map_err(|err| io_err(&guard.path, err))?;
    guard.committed = true;
    Ok(())
}

async fn copy_limited(
    file: &mut tokio::fs::File,
    reader: &mut (impl AsyncRead + Unpin),
    max_bytes: u64,
    part: &Path,
) -> Result<(), AppError> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = reader
            .read(&mut buf)
            .await
            .map_err(|err| AppError::new(ErrorCode::Internal, format!("đọc upload: {err}")))?;
        if n == 0 {
            return Ok(());
        }
        let next = total.saturating_add(n as u64);
        if next > max_bytes {
            return Err(too_large(next));
        }
        file.write_all(&buf[..n])
            .await
            .map_err(|err| io_err(part, err))?;
        total = next;
    }
}

/// Xóa `.part` khi upload không đổi tên xong, kể cả lúc future bị hủy.
struct PartGuard {
    path: PathBuf,
    committed: bool,
}

impl Drop for PartGuard {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn validate_file_name(name: &str) -> Result<(), AppError> {
    // Một thành phần thôi: `a/b` sẽ ghi ra ngoài thư mục người dùng đã chọn.
    let bad = name.is_empty()
        || name == "."
        || name == ".."
        || name.chars().any(|c| matches!(c, '/' | '\\' | '\0' | ':'));
    if bad {
        Err(invalid("tên file không hợp lệ"))
    } else {
        Ok(())
    }
}

fn child_path(dir: &str, name: &str) -> String {
    if collapses_to_root(dir) {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

fn is_root_server_jar(user_path: &str) -> bool {
    matches!(normalized(user_path).as_deref(), Some([SERVER_JAR]))
}

fn collapses_to_root(user_path: &str) -> bool {
    matches!(normalized(user_path).as_deref(), Some([]))
}

/// `None` khi `..` thoát gốc. Gộp `.` và `..` giống `SafeRoot` để `./server.jar` vẫn bị chặn.
fn normalized(user_path: &str) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    for part in user_path.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    Some(parts)
}

pub(super) fn skips_list_entry(err: &AppError) -> bool {
    matches!(
        err.code,
        ErrorCode::PathOutsideRoot | ErrorCode::InvalidInput
    )
}

pub(super) fn outside(user_path: &str) -> AppError {
    AppError::new(
        ErrorCode::PathOutsideRoot,
        format!("đường dẫn tuyệt đối `{user_path}`"),
    )
}

pub(super) fn io_err(path: &Path, err: std::io::Error) -> AppError {
    match err.kind() {
        std::io::ErrorKind::NotFound => AppError::not_found(path.display()),
        std::io::ErrorKind::AlreadyExists => exists(&path.display().to_string()),
        _ => PlatformError::io(path, err).into(),
    }
}

pub(super) fn plat(err: PlatformError) -> AppError {
    match err {
        PlatformError::Io { path, source } if source.kind() == std::io::ErrorKind::NotFound => {
            AppError::not_found(path.display())
        }
        other => other.into(),
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    match tokio::task::spawn_blocking(work).await {
        Ok(result) => result,
        Err(err) => Err(AppError::new(
            ErrorCode::Internal,
            format!("tác vụ đĩa dừng: {err}"),
        )),
    }
}

pub(super) fn too_large(bytes: u64) -> AppError {
    AppError::new(ErrorCode::FileTooLarge, format!("{bytes} byte"))
}

pub(super) fn not_text() -> AppError {
    AppError::new(ErrorCode::FileNotText, "không phải văn bản UTF-8")
}

pub(super) fn changed() -> AppError {
    AppError::new(ErrorCode::FileChanged, "nội dung đã đổi kể từ lúc đọc")
}

pub(super) fn exists(path: &str) -> AppError {
    AppError::new(ErrorCode::FileExists, format!("đã có `{path}`"))
}

pub(super) fn invalid(detail: &str) -> AppError {
    AppError::new(ErrorCode::InvalidInput, detail)
}

fn busy_jar() -> AppError {
    AppError::new(
        ErrorCode::ServerBusy,
        "không sửa server.jar khi server đang chạy",
    )
}
