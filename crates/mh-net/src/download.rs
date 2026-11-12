use std::path::{Path, PathBuf};

use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

use crate::client::hex;
use crate::{HttpClient, NetError};

/// Dưới mức này thì không tải gì nữa (spec P1, mục 4.5 `NOT_ENOUGH_DISK`).
pub const MIN_FREE_BYTES: u64 = 1024 * 1024 * 1024;

/// Hash mà file tải về phải khớp. Chuỗi hex đã kiểm tra, nên dùng làm tên file cache an toàn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checksum {
    /// Chỉ Purpur dùng (spec P2, mục 5.4). md5 không chống được người cố ý sửa file, nhưng file
    /// tải qua HTTPS từ chính API của Purpur, nên ở đây chỉ để bắt file hỏng khi tải.
    Md5(String),
    Sha1(String),
    Sha256(String),
}

impl Checksum {
    pub fn md5(hex: &str) -> Option<Self> {
        valid_hex(hex, 32).map(Self::Md5)
    }

    pub fn sha1(hex: &str) -> Option<Self> {
        valid_hex(hex, 40).map(Self::Sha1)
    }

    pub fn sha256(hex: &str) -> Option<Self> {
        valid_hex(hex, 64).map(Self::Sha256)
    }

    fn algo(&self) -> &'static str {
        match self {
            Self::Md5(_) => "md5",
            Self::Sha1(_) => "sha1",
            Self::Sha256(_) => "sha256",
        }
    }

    pub fn hex(&self) -> &str {
        match self {
            Self::Md5(h) | Self::Sha1(h) | Self::Sha256(h) => h,
        }
    }
}

fn valid_hex(hex: &str, len: usize) -> Option<String> {
    (hex.len() == len && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_lowercase())
}

/// Một file cần tải.
#[derive(Debug, Clone, Copy)]
pub struct Download<'a> {
    pub url: &'a str,
    pub checksum: &'a Checksum,
    pub dest: &'a Path,
    /// Kích thước biết trước (từ API), để hiện tiến độ khi server không gửi `Content-Length`.
    pub size: Option<u64>,
}

enum Hasher {
    Md5(Md5),
    Sha1(Sha1),
    Sha256(Sha256),
}

impl Hasher {
    fn new(checksum: &Checksum) -> Self {
        match checksum {
            Checksum::Md5(_) => Self::Md5(Md5::new()),
            Checksum::Sha1(_) => Self::Sha1(Sha1::new()),
            Checksum::Sha256(_) => Self::Sha256(Sha256::new()),
        }
    }

    fn update(&mut self, data: &[u8]) {
        match self {
            Self::Md5(h) => h.update(data),
            Self::Sha1(h) => h.update(data),
            Self::Sha256(h) => h.update(data),
        }
    }

    fn finish(self) -> String {
        match self {
            Self::Md5(h) => hex(&h.finalize()),
            Self::Sha1(h) => hex(&h.finalize()),
            Self::Sha256(h) => hex(&h.finalize()),
        }
    }
}

/// Hash của `data` theo cùng thuật toán với `checksum`, dạng hex chữ thường.
pub(crate) fn digest_hex(checksum: &Checksum, data: &[u8]) -> String {
    let mut hasher = Hasher::new(checksum);
    hasher.update(data);
    hasher.finish()
}

impl HttpClient {
    /// Tải `url` về `dest`, gọi `progress(done, total)` sau mỗi đoạn.
    ///
    /// Dữ liệu ghi vào `dest.part` và hash được tính trong lúc tải; chỉ khi khớp mới đổi tên
    /// thành `dest`. File đã tải được giữ trong `cache/<algo>/<2 ký tự>/<hash>`, nên lần sau
    /// cùng hash thì chép từ cache, không tải lại.
    pub async fn download(
        &self,
        req: Download<'_>,
        mut progress: impl FnMut(u64, Option<u64>),
        cancel: &CancellationToken,
    ) -> Result<(), NetError> {
        if cancel.is_cancelled() {
            return Err(NetError::Cancelled);
        }
        let dir = req.dest.parent().unwrap_or(Path::new("."));
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| NetError::io(dir, e))?;
        let cached = self.cache_path(req.checksum);
        if let Ok(meta) = tokio::fs::metadata(&cached).await {
            copy_replace(&cached, req.dest).await?;
            progress(meta.len(), Some(meta.len()));
            return Ok(());
        }
        self.check_free_space(dir)?;
        let part = part_path(req.dest);
        let result = self.fetch_to_part(&req, &part, &mut progress, cancel).await;
        if let Err(err) = result {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(err);
        }
        if let Err(err) = copy_replace(&part, &cached).await {
            tracing::warn!(url = req.url, error = %err, "không lưu được file vào cache");
        }
        tokio::fs::rename(&part, req.dest)
            .await
            .map_err(|e| NetError::io(req.dest, e))
    }

    async fn fetch_to_part(
        &self,
        req: &Download<'_>,
        part: &Path,
        progress: &mut impl FnMut(u64, Option<u64>),
        cancel: &CancellationToken,
    ) -> Result<(), NetError> {
        let mut resp = tokio::select! {
            () = cancel.cancelled() => return Err(NetError::Cancelled),
            resp = self.send(req.url) => resp?,
        };
        let total = resp.content_length().or(req.size);
        let mut file = tokio::fs::File::create(part)
            .await
            .map_err(|e| NetError::io(part, e))?;
        let mut hasher = Hasher::new(req.checksum);
        let mut done = 0u64;
        loop {
            let chunk = tokio::select! {
                () = cancel.cancelled() => return Err(NetError::Cancelled),
                chunk = resp.chunk() => chunk.map_err(|e| download_error(req.url, &e))?,
            };
            let Some(chunk) = chunk else { break };
            file.write_all(&chunk)
                .await
                .map_err(|e| NetError::io(part, e))?;
            hasher.update(&chunk);
            done += chunk.len() as u64;
            progress(done, total);
        }
        // Người dùng có thể hủy ngay trong `progress` của đoạn cuối.
        if cancel.is_cancelled() {
            return Err(NetError::Cancelled);
        }
        file.sync_all().await.map_err(|e| NetError::io(part, e))?;
        let actual = hasher.finish();
        if actual != req.checksum.hex() {
            return Err(NetError::ChecksumMismatch {
                url: req.url.to_owned(),
                expected: req.checksum.hex().to_owned(),
                actual,
            });
        }
        Ok(())
    }

    fn check_free_space(&self, dir: &Path) -> Result<(), NetError> {
        let available = self.free_space.as_ref().and_then(|f| f(dir));
        match available {
            Some(available) if available < MIN_FREE_BYTES => {
                Err(NetError::NotEnoughDisk { available })
            }
            _ => Ok(()),
        }
    }

    fn cache_path(&self, checksum: &Checksum) -> PathBuf {
        let hex = checksum.hex();
        self.config
            .cache_dir
            .join(checksum.algo())
            .join(&hex[..2])
            .join(hex)
    }
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

/// Chép qua file tạm rồi đổi tên, để `to` không bao giờ là file chép dở.
async fn copy_replace(from: &Path, to: &Path) -> Result<(), NetError> {
    if let Some(dir) = to.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| NetError::io(dir, e))?;
    }
    let tmp = to.with_extension("tmp");
    tokio::fs::copy(from, &tmp)
        .await
        .map_err(|e| NetError::io(&tmp, e))?;
    tokio::fs::rename(&tmp, to)
        .await
        .map_err(|e| NetError::io(to, e))
}

fn download_error(url: &str, err: &reqwest::Error) -> NetError {
    NetError::Download {
        url: url.to_owned(),
        reason: err.to_string(),
    }
}
