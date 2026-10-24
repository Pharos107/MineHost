//! Tải và quản lý Temurin JRE (spec P1, mục 5.4).
//!
//! P1 chỉ dùng Java do MineHost tải vào `java/<major>/`, không dò Java có sẵn trên máy (mục 3.2).

mod adoptium;
mod error;
mod extract;
mod probe;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use mh_net::{CancellationToken, Download, HttpClient};

pub use adoptium::API_BASE;
pub use error::JavaError;
pub use probe::{ProbeFn, java_version};

/// Một bản Java đã cài và chạy thử được.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaRuntime {
    pub major: u16,
    pub home: PathBuf,
    pub java_bin: PathBuf,
    /// Chuỗi phiên bản từ `java -version`, ví dụ `21.0.5`.
    pub version: String,
}

pub struct JavaManager {
    http: Arc<HttpClient>,
    java_dir: PathBuf,
    api_base: String,
    probe: ProbeFn,
    /// Khóa theo `major`: hai server cùng cần Java 21 thì chỉ tải một lần.
    locks: Mutex<HashMap<u16, Arc<tokio::sync::Mutex<()>>>>,
}

impl std::fmt::Debug for JavaManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JavaManager")
            .field("java_dir", &self.java_dir)
            .field("api_base", &self.api_base)
            .finish_non_exhaustive()
    }
}

/// Đường dẫn `java` trong một thư mục JRE.
pub fn java_bin(home: &Path) -> PathBuf {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    home.join("bin").join(exe)
}

impl JavaManager {
    /// `java_dir` là thư mục `java/` trong thư mục dữ liệu của MineHost.
    pub fn new(http: Arc<HttpClient>, java_dir: impl Into<PathBuf>) -> Self {
        Self {
            http,
            java_dir: java_dir.into(),
            api_base: API_BASE.to_owned(),
            probe: Arc::new(java_version),
            locks: Mutex::new(HashMap::new()),
        }
    }

    /// Dùng trong test để trỏ tới server giả.
    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
    }

    /// Thay cách chạy thử Java; test dùng để khỏi cần Java thật.
    pub fn with_probe(mut self, probe: ProbeFn) -> Self {
        self.probe = probe;
        self
    }

    /// Bảo đảm có Java `major`: dùng bản đã cài nếu chạy được, nếu không thì tải từ Adoptium.
    pub async fn ensure(
        &self,
        major: u16,
        progress: impl FnMut(u64, Option<u64>),
        cancel: &CancellationToken,
    ) -> Result<JavaRuntime, JavaError> {
        let lock = self.lock_for(major);
        let _guard = lock.lock().await;
        let home = self.java_dir.join(major.to_string());
        if home.exists() {
            match self.check(major, &home).await {
                Ok(runtime) => return Ok(runtime),
                Err(err) => {
                    tracing::warn!(major, error = %err, "Java đã cài không chạy được, cài lại");
                    tokio::fs::remove_dir_all(&home)
                        .await
                        .map_err(JavaError::install)?;
                }
            }
        }
        self.install(major, &home, progress, cancel).await
    }

    async fn install(
        &self,
        major: u16,
        home: &Path,
        progress: impl FnMut(u64, Option<u64>),
        cancel: &CancellationToken,
    ) -> Result<JavaRuntime, JavaError> {
        let package = adoptium::latest_package(&self.http, &self.api_base, major).await?;
        tokio::fs::create_dir_all(&self.java_dir)
            .await
            .map_err(JavaError::install)?;
        // Thư mục tạm nằm cạnh đích để `rename` không phải chép qua ổ khác; tự xóa khi lỗi.
        let tmp = tempfile::Builder::new()
            .prefix(&format!(".tmp-{major}-"))
            .tempdir_in(&self.java_dir)
            .map_err(JavaError::install)?;
        let archive = tmp.path().join("package");
        let req = Download {
            url: &package.link,
            checksum: &package.checksum,
            dest: &archive,
            size: Some(package.size),
        };
        self.http.download(req, progress, cancel).await?;
        let extracted = tmp.path().join("x");
        let kind = package.kind;
        let root = tokio::task::spawn_blocking(move || extract::unpack(kind, &archive, &extracted))
            .await
            .map_err(JavaError::install)??;
        // Chạy thử trước khi đổi tên, để `java/<major>/` chỉ tồn tại khi Java dùng được.
        let runtime = self.check(major, &root).await?;
        tokio::fs::rename(&root, home)
            .await
            .map_err(JavaError::install)?;
        tracing::info!(major, version = runtime.version, "đã cài Java");
        Ok(JavaRuntime {
            home: home.to_path_buf(),
            java_bin: java_bin(home),
            ..runtime
        })
    }

    async fn check(&self, major: u16, home: &Path) -> Result<JavaRuntime, JavaError> {
        let bin = java_bin(home);
        let probe = Arc::clone(&self.probe);
        let probe_bin = bin.clone();
        let version = tokio::task::spawn_blocking(move || probe(&probe_bin))
            .await
            .map_err(JavaError::install)?
            .map_err(JavaError::Install)?;
        Ok(JavaRuntime {
            major,
            home: home.to_path_buf(),
            java_bin: bin,
            version,
        })
    }

    fn lock_for(&self, major: u16) -> Arc<tokio::sync::Mutex<()>> {
        // Mutex chỉ bị poison khi một luồng panic lúc đang giữ; khi đó vẫn dùng tiếp map bên trong.
        let mut locks = self.locks.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(locks.entry(major).or_default())
    }
}
