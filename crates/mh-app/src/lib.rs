//! Use case của MineHost (spec P1, mục 5.7).
//!
//! `mh-api` và CLI chỉ gọi vào đây, không gọi thẳng các crate tầng dưới. Đây là nơi duy nhất
//! ghép nhiều crate lại với nhau.

mod create;
#[cfg(feature = "e2e")]
pub mod e2e;
mod error;
pub mod jobs;
mod launcher;
mod lifecycle;
mod servers;
pub mod sources;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mh_core::{GamePort, LoaderKind, McVersion, ServerId, ServerName};
use mh_java::JavaManager;
use mh_net::{FreeSpaceFn, HttpClient, NetConfig};
use mh_platform::{DataDirs, SystemInfo};
use mh_runtime::ProcessRuntime;
use mh_store::ServerStore;
use mh_supervisor::SupervisorConfig;

pub use create::{CreateRequest, Created};
pub use error::AppError;
pub use jobs::{JobId, JobRunner, JobSnapshot, JobState};
pub use mh_supervisor::{Status, SupervisorEvent, SupervisorHandle};
pub use servers::{ServerView, SettingsPatch};
pub use sources::{CatalogVersion, JavaSource, ServerCatalog};

/// Phiên bản controller, lấy từ `Cargo.toml` của workspace.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Tối đa 2 lượt tải cùng lúc (spec P1, mục 5.7).
const MAX_DOWNLOADS: usize = 2;

/// Cổng có còn trống không (thử bind `0.0.0.0:<port>`). Test thay bằng hàm giả.
pub type PortProbe = Arc<dyn Fn(u16) -> bool + Send + Sync>;

/// Những thứ thay được khi test.
pub struct AppDeps {
    pub catalog: Arc<dyn ServerCatalog>,
    pub java: Arc<dyn JavaSource>,
    pub port_free: PortProbe,
    pub free_space: FreeSpaceFn,
    pub total_memory_mb: u64,
    pub supervisor: SupervisorConfig,
}

impl std::fmt::Debug for AppDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppDeps")
            .field("total_memory_mb", &self.total_memory_mb)
            .field("supervisor", &self.supervisor)
            .finish_non_exhaustive()
    }
}

/// Sự kiện cấp app cho WebSocket. Chỉ báo "server này đã đổi"; người nghe tự đọc lại `get`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEvent {
    ServerChanged(ServerId),
}

/// Server đang được Job `create` cài, chưa có `instance.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installing {
    pub name: ServerName,
    pub loader: LoaderKind,
    pub minecraft: McVersion,
    pub port: GamePort,
    pub job_id: Option<JobId>,
}

/// Trạng thái dùng chung của controller. Bọc trong `Arc` để chia sẻ giữa các request.
pub struct App {
    dirs: DataDirs,
    store: ServerStore,
    catalog: Arc<dyn ServerCatalog>,
    java: Arc<dyn JavaSource>,
    runtime: Arc<ProcessRuntime>,
    jobs: JobRunner,
    supervisors: Mutex<HashMap<ServerId, SupervisorHandle>>,
    installing: Mutex<HashMap<ServerId, Installing>>,
    port_free: PortProbe,
    free_space: FreeSpaceFn,
    total_memory_mb: u64,
    supervisor_config: SupervisorConfig,
    shutdown: tokio::sync::Notify,
    events: tokio::sync::broadcast::Sender<AppEvent>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("dirs", &self.dirs)
            .finish_non_exhaustive()
    }
}

impl App {
    /// Dùng nguồn thật: Mojang, Paper, Adoptium. Phải gọi trong runtime tokio.
    pub fn new(dirs: DataDirs) -> Result<Self, AppError> {
        let (port_free, free_space, total_memory_mb) = host_deps_parts();
        let http =
            HttpClient::new(NetConfig::new(dirs.cache()))?.with_free_space(Arc::clone(&free_space));
        let http = Arc::new(http);
        let deps = AppDeps {
            catalog: Arc::new(sources::OnlineCatalog::new(Arc::clone(&http))),
            java: Arc::new(JavaManager::new(http, dirs.java())),
            port_free,
            free_space,
            total_memory_mb,
            supervisor: SupervisorConfig::default(),
        };
        Self::with_deps(dirs, deps)
    }

    pub fn with_deps(dirs: DataDirs, deps: AppDeps) -> Result<Self, AppError> {
        Ok(Self {
            store: ServerStore::new(dirs.servers(), dirs.trash()),
            dirs,
            catalog: deps.catalog,
            java: deps.java,
            runtime: Arc::new(ProcessRuntime::new()?),
            jobs: JobRunner::new(MAX_DOWNLOADS),
            supervisors: Mutex::new(HashMap::new()),
            installing: Mutex::new(HashMap::new()),
            port_free: deps.port_free,
            free_space: deps.free_space,
            total_memory_mb: deps.total_memory_mb,
            supervisor_config: deps.supervisor,
            shutdown: tokio::sync::Notify::new(),
            events: tokio::sync::broadcast::channel(256).0,
        })
    }

    pub fn dirs(&self) -> &DataDirs {
        &self.dirs
    }

    pub fn jobs(&self) -> &JobRunner {
        &self.jobs
    }

    /// Thông tin máy. Chặn luồng vài mili giây, nên gọi trong `spawn_blocking`.
    pub fn system_info(&self) -> SystemInfo {
        mh_platform::system::probe(self.dirs.root())
    }

    pub fn suggested_memory(&self) -> mh_core::MemoryLimits {
        mh_core::suggest_memory_mb(self.total_memory_mb)
    }

    pub async fn catalog_versions(
        &self,
        loader: LoaderKind,
    ) -> Result<Vec<CatalogVersion>, AppError> {
        self.catalog.versions(loader).await
    }

    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<AppEvent> {
        self.events.subscribe()
    }

    pub(crate) fn notify_changed(&self, id: ServerId) {
        // Không ai nghe thì thôi.
        let _ = self.events.send(AppEvent::ServerChanged(id));
    }

    pub fn lan_addresses(&self) -> Vec<std::net::Ipv4Addr> {
        mh_platform::system::lan_ipv4()
    }

    /// Nút "Thoát" trong UI: báo cho binary dừng web server rồi gọi `shutdown_all`.
    pub fn request_shutdown(&self) {
        // `notify_one` giữ lại tín hiệu nếu chưa ai chờ, nên không bị mất.
        self.shutdown.notify_one();
    }

    pub async fn shutdown_requested(&self) {
        self.shutdown.notified().await;
    }

    /// Thời gian chờ một server dừng khi thoát MineHost: hạn `stop` cộng thêm một chút.
    fn shutdown_timeout(&self) -> Duration {
        self.supervisor_config.stop_timeout + Duration::from_secs(10)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // Chỉ bị poison khi một luồng panic lúc đang giữ; dữ liệu bên trong vẫn dùng được.
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Cổng, dung lượng ổ và RAM của máy đang chạy. Catalog và Java do caller chọn,
/// để chế độ e2e chỉ thay hai thứ đó và vẫn dùng probe thật.
pub(crate) fn host_deps_parts() -> (PortProbe, FreeSpaceFn, u64) {
    (
        Arc::new(port_is_free),
        Arc::new(mh_platform::system::free_space),
        mh_platform::system::total_memory_bytes() / (1024 * 1024),
    )
}

fn port_is_free(port: u16) -> bool {
    std::net::TcpListener::bind(("0.0.0.0", port)).is_ok()
}
