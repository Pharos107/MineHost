use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use crate::download::digest_hex;
use crate::{Checksum, NetError};

/// PaperMC yêu cầu User-Agent có thông tin liên hệ; thiếu thì có thể bị chặn.
pub const USER_AGENT: &str = concat!(
    "MineHost/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Pharos107/MineHost)"
);

/// Cho biết còn bao nhiêu byte trống trên ổ chứa một đường dẫn.
///
/// `mh-net` không phụ thuộc `mh-platform`, nên crate ghép nối (`mh-app`) truyền hàm này vào.
pub type FreeSpaceFn = Arc<dyn Fn(&Path) -> Option<u64> + Send + Sync>;

#[derive(Debug, Clone)]
pub struct NetConfig {
    /// Thư mục `cache/` của MineHost.
    pub cache_dir: PathBuf,
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    /// Thời gian chờ trước mỗi lần thử lại; số phần tử là số lần thử lại.
    pub retry_delays: Vec<Duration>,
}

impl NetConfig {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(30),
            retry_delays: [500, 1000, 2000].map(Duration::from_millis).to_vec(),
        }
    }
}

/// Kết quả `get_json`. `stale` là `true` khi mạng lỗi và phải dùng bản lưu lần trước.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched<T> {
    pub value: T,
    pub stale: bool,
}

/// Thân response theo URL, kèm thời điểm tải để tính TTL.
type MemoryCache = HashMap<String, (Instant, Arc<[u8]>)>;

pub struct HttpClient {
    pub(crate) http: reqwest::Client,
    pub(crate) config: NetConfig,
    memory: Mutex<MemoryCache>,
    pub(crate) free_space: Option<FreeSpaceFn>,
}

impl fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl HttpClient {
    pub fn new(config: NetConfig) -> Result<Self, NetError> {
        // reqwest được build không kèm crypto provider (aws-lc vướng giấy phép OpenSSL), nên cài
        // ring làm mặc định cho cả process. Gọi lại khi đã cài thì trả lỗi, bỏ qua được.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(config.connect_timeout)
            .read_timeout(config.read_timeout)
            .build()
            .map_err(|e| NetError::Unavailable {
                url: String::new(),
                reason: e.to_string(),
            })?;
        Ok(Self {
            http,
            config,
            memory: Mutex::new(HashMap::new()),
            free_space: None,
        })
    }

    /// Bật kiểm tra ổ đĩa trống trước khi tải.
    pub fn with_free_space(mut self, free_space: FreeSpaceFn) -> Self {
        self.free_space = Some(free_space);
        self
    }

    /// GET JSON, cache trong RAM `ttl` giây. Bản tốt gần nhất được lưu vào `cache/meta/` để
    /// vẫn tạo server được khi mạng chập chờn.
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        ttl: Duration,
    ) -> Result<Fetched<T>, NetError> {
        self.fetch_json(url, ttl, None).await
    }

    /// Như `get_json`, thêm kiểm tra hash của thân response (ví dụ `sha1` mà manifest Mojang
    /// ghi cho JSON của từng phiên bản).
    pub async fn get_json_checked<T: DeserializeOwned>(
        &self,
        url: &str,
        ttl: Duration,
        checksum: &Checksum,
    ) -> Result<Fetched<T>, NetError> {
        self.fetch_json(url, ttl, Some(checksum)).await
    }

    async fn fetch_json<T: DeserializeOwned>(
        &self,
        url: &str,
        ttl: Duration,
        checksum: Option<&Checksum>,
    ) -> Result<Fetched<T>, NetError> {
        let fresh = |value| Fetched {
            value,
            stale: false,
        };
        if let Some(body) = self.memory_hit(url, ttl) {
            return decode(url, &body, checksum).map(fresh);
        }
        let err = match self.fetch_bytes(url).await {
            Ok(body) => {
                // Kiểm tra trước khi lưu, để dữ liệu hỏng không bao giờ vào cache.
                let value = decode(url, &body, checksum)?;
                self.remember(url, body).await;
                return Ok(fresh(value));
            }
            Err(err) => err,
        };
        if !err.is_network() {
            return Err(err);
        }
        let Ok(body) = tokio::fs::read(self.meta_path(url)).await else {
            return Err(err);
        };
        tracing::warn!(url, error = %err, "mạng lỗi, dùng bản lưu lần trước");
        decode(url, &body, checksum).map(|value| Fetched { value, stale: true })
    }

    /// GET có retry khi timeout, lỗi kết nối, 5xx hoặc 429. Chỉ trả response 2xx.
    pub(crate) async fn send(&self, url: &str) -> Result<reqwest::Response, NetError> {
        let mut attempt = 0;
        loop {
            let err = match self.http.get(url).send().await {
                Ok(resp) if resp.status().is_success() => return Ok(resp),
                Ok(resp) if !is_retryable(resp.status()) => {
                    return Err(NetError::Status {
                        url: url.to_owned(),
                        status: resp.status().as_u16(),
                    });
                }
                Ok(resp) => unavailable(url, &resp.status()),
                // URL sai thì thử lại cũng vô ích.
                Err(e) if e.is_builder() => return Err(unavailable(url, &e)),
                Err(e) => unavailable(url, &e),
            };
            let Some(delay) = self.config.retry_delays.get(attempt) else {
                return Err(err);
            };
            tracing::warn!(url, attempt, error = %err, "gọi HTTP lỗi, thử lại sau {delay:?}");
            tokio::time::sleep(*delay).await;
            attempt += 1;
        }
    }

    async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, NetError> {
        let resp = self.send(url).await?;
        let body = resp.bytes().await.map_err(|e| unavailable(url, &e))?;
        Ok(body.to_vec())
    }

    fn memory_hit(&self, url: &str, ttl: Duration) -> Option<Arc<[u8]>> {
        let memory = self.memory.lock().ok()?;
        let (at, body) = memory.get(url)?;
        (at.elapsed() < ttl).then(|| Arc::clone(body))
    }

    async fn remember(&self, url: &str, body: Vec<u8>) {
        let path = self.meta_path(url);
        if let Err(err) = write_replace(&path, &body).await {
            tracing::warn!(url, error = %err, "không lưu được bản cache meta");
        }
        if let Ok(mut memory) = self.memory.lock() {
            memory.insert(url.to_owned(), (Instant::now(), body.into()));
        }
    }

    fn meta_path(&self, url: &str) -> PathBuf {
        let key = hex(&Sha256::digest(url.as_bytes()));
        self.config
            .cache_dir
            .join("meta")
            .join(format!("{key}.json"))
    }
}

fn decode<T: DeserializeOwned>(
    url: &str,
    body: &[u8],
    checksum: Option<&Checksum>,
) -> Result<T, NetError> {
    if let Some(expected) = checksum {
        let actual = digest_hex(expected, body);
        if actual != expected.hex() {
            return Err(NetError::ChecksumMismatch {
                url: url.to_owned(),
                expected: expected.hex().to_owned(),
                actual,
            });
        }
    }
    serde_json::from_slice(body).map_err(|e| {
        tracing::error!(url, error = %e, "JSON từ API không đúng cấu trúc mong đợi");
        NetError::Decode {
            url: url.to_owned(),
            reason: e.to_string(),
        }
    })
}

fn is_retryable(status: reqwest::StatusCode) -> bool {
    status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
}

fn unavailable(url: &str, reason: &dyn fmt::Display) -> NetError {
    NetError::Unavailable {
        url: url.to_owned(),
        reason: reason.to_string(),
    }
}

/// Ghi vào file tạm cạnh đích rồi đổi tên, để người đọc không bao giờ thấy file ghi dở.
pub(crate) async fn write_replace(path: &Path, contents: &[u8]) -> Result<(), NetError> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| NetError::io(dir, e))?;
    }
    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, contents)
        .await
        .map_err(|e| NetError::io(&tmp, e))?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|e| NetError::io(path, e))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
