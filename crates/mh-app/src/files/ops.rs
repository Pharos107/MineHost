//! Thao tác nhiều mục của trang Tệp: tìm, zip tải xuống, nén, giải nén (spec trang Tệp, mục 3).

use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use mh_core::{ErrorCode, ServerId};
use tokio::sync::mpsc;
use zip::ZipWriter;

use super::archive::{self, Progress};
use super::search::{self, SEARCH_MAX_RESULTS, SEARCH_MAX_VISITED, SearchResult};
use super::{AppError, blocking, disk};
use crate::App;
use crate::jobs::{JobContext, JobId, JobKind, JobStep};

/// Mỗi khúc gửi cho client khi stream zip tải xuống.
const STREAM_CHUNK: usize = 256 * 1024;
/// Số khúc nằm chờ client đọc; client chậm thì luồng nén cũng chờ, không gom vào RAM.
const STREAM_BACKLOG: usize = 4;

pub type ZipStream = mpsc::Receiver<io::Result<Vec<u8>>>;

impl App {
    pub async fn search_files(
        &self,
        id: ServerId,
        path: &str,
        query: &str,
    ) -> Result<SearchResult, AppError> {
        let (store, path, query) = (self.store.clone(), path.to_owned(), query.to_owned());
        blocking(move || {
            let root = disk::open_root(&store, id)?;
            search::search_sync(
                &root,
                &path,
                &query,
                (SEARCH_MAX_RESULTS, SEARCH_MAX_VISITED),
            )
        })
        .await
    }

    /// Lập danh sách trước (lỗi trả về như route thường), rồi nén thẳng vào kênh: không file tạm.
    pub async fn zip_download(
        &self,
        id: ServerId,
        paths: Vec<String>,
    ) -> Result<(String, ZipStream), AppError> {
        let name = download_name(&paths);
        let store = self.store.clone();
        let plan =
            blocking(move || archive::plan_zip(&disk::open_root(&store, id)?, &paths)).await?;
        let (tx, rx) = mpsc::channel(STREAM_BACKLOG);
        tokio::task::spawn_blocking(move || {
            let writer = ChannelWriter {
                tx: tx.clone(),
                buf: Vec::with_capacity(STREAM_CHUNK),
            };
            let mut zip = ZipWriter::new_stream(writer);
            let result = archive::write_zip(&plan, &mut zip, &mut |_| Ok(()))
                .and_then(|()| {
                    zip.finish()
                        .map_err(|err| internal(format!("kết thúc zip: {err}")))
                })
                .and_then(|mut stream| {
                    stream
                        .flush()
                        .map_err(|err| internal(format!("gửi zip: {err}")))
                });
            if let Err(err) = result {
                tracing::warn!(server = %id, error = %err, "dừng stream zip tải xuống");
                // Client nhận lỗi giữa chừng thay vì một zip cụt trông như đủ.
                let _ = tx.blocking_send(Err(io::Error::other(err.detail)));
            }
        });
        Ok((name, rx))
    }

    /// Kiểm tra đích và lập danh sách ngay, lỗi trả về luôn; phần nén chạy thành Job.
    pub async fn compress_files(
        self: &Arc<Self>,
        id: ServerId,
        paths: Vec<String>,
        dest: &str,
    ) -> Result<JobId, AppError> {
        self.guard_jar(id, dest)?;
        let (store, dest) = (self.store.clone(), dest.to_owned());
        let (plan, target) = blocking(move || {
            let root = disk::open_root(&store, id)?;
            Ok((
                archive::plan_zip(&root, &paths)?,
                archive::compress_target(&root, &dest)?,
            ))
        })
        .await?;
        Ok(self
            .jobs
            .spawn(JobKind::CompressFiles, Some(id), move |ctx| async move {
                ctx.step(JobStep::Compress, 1, 1);
                let total = plan.total;
                with_progress(&ctx, total, move |progress| {
                    archive::compress_sync(&plan, &target, progress)
                })
                .await
            }))
    }

    /// Kiểm tra an toàn và trùng tên ngay (spec trang Tệp, mục 3); phần ghi chạy thành Job.
    pub async fn extract_files(
        self: &Arc<Self>,
        id: ServerId,
        path: &str,
        dest: &str,
    ) -> Result<JobId, AppError> {
        let (store, path, dest) = (self.store.clone(), path.to_owned(), dest.to_owned());
        let plan = blocking(move || {
            let root = disk::open_root(&store, id)?;
            let free = mh_platform::system::free_space(root.path());
            archive::plan_extract(&root, &path, &dest, free)
        })
        .await?;
        Ok(self
            .jobs
            .spawn(JobKind::ExtractFiles, Some(id), move |ctx| async move {
                ctx.step(JobStep::Unpack, 1, 1);
                let total = plan.total;
                with_progress(&ctx, total, move |progress| {
                    archive::extract_sync(plan, progress)
                })
                .await
            }))
    }
}

/// Chạy `work` trong `spawn_blocking`, chuyển tiến độ về Job; Job bị hủy thì `work` dừng ở khúc kế.
async fn with_progress(
    ctx: &JobContext,
    total: u64,
    work: impl FnOnce(Progress<'_>) -> Result<(), AppError> + Send + 'static,
) -> Result<(), AppError> {
    let cancel = ctx.cancel.clone();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let task = tokio::task::spawn_blocking(move || {
        work(&mut |done| {
            if cancel.is_cancelled() {
                return Err(AppError::cancelled());
            }
            let _ = tx.send(done);
            Ok(())
        })
    });
    ctx.bytes(0, Some(total));
    // Kênh đóng khi `work` xong (tx bị drop), nên vòng này luôn dừng.
    while let Some(done) = rx.recv().await {
        ctx.bytes(done, Some(total));
    }
    task.await
        .map_err(|err| internal(format!("tác vụ zip dừng: {err}")))?
}

fn internal(detail: String) -> AppError {
    AppError::new(ErrorCode::Internal, detail)
}

/// Một mục: `<tên>.zip`. Nhiều mục: tên thư mục chứa chúng, ở gốc là `server.zip`.
fn download_name(paths: &[String]) -> String {
    let base = match paths {
        [one] => Path::new(one).file_name(),
        [first, ..] => Path::new(first).parent().and_then(Path::file_name),
        [] => None,
    };
    let base = base.and_then(|name| name.to_str()).unwrap_or("server");
    format!("{base}.zip")
}

/// `Write` gửi từng khúc qua kênh. Client ngắt thì kênh đóng, ghi lỗi và luồng nén dừng.
struct ChannelWriter {
    tx: mpsc::Sender<io::Result<Vec<u8>>>,
    buf: Vec<u8>,
}

impl ChannelWriter {
    fn send(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let chunk = std::mem::replace(&mut self.buf, Vec::with_capacity(STREAM_CHUNK));
        self.tx
            .blocking_send(Ok(chunk))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "người tải đã ngắt"))
    }
}

impl Write for ChannelWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(data);
        if self.buf.len() >= STREAM_CHUNK {
            self.send()?;
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.send()
    }
}

#[cfg(test)]
mod tests {
    use super::download_name;

    #[test]
    fn ten_zip_tai_xuong() {
        assert_eq!(download_name(&["world".into()]), "world.zip");
        assert_eq!(
            download_name(&["plugins/a.jar".into(), "plugins/b.jar".into()]),
            "plugins.zip"
        );
        assert_eq!(download_name(&["a".into(), "b".into()]), "server.zip");
    }
}
