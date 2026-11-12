use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use mh_app::App;
use mh_core::ErrorCode;
use mh_platform::DataDirs;
use tokio::net::TcpListener;

use crate::config::Config;
use crate::instance::InstanceLock;
use crate::lan::LanManager;

/// Gọi một lần khi controller đã mở cổng và tạo xong `App`; khay dùng để biết url và gọi Thoát.
pub type ReadyHook = Box<dyn FnOnce(&Arc<App>, &str) + Send>;

/// Web server tắt xong các kết nối trong chừng này; quá thì thoát luôn.
const HTTP_DRAIN: Duration = Duration::from_secs(5);

/// Chạy API và UI tới khi có yêu cầu thoát: Ctrl+C, `SIGTERM`, nút Thoát trong UI hoặc ở khay.
/// Mọi đường đều đi qua `shutdown_all`, nên không server nào bị kill khi người dùng thoát.
pub async fn run(
    dirs: DataDirs,
    config: Config,
    allow_browser: bool,
    lock: &InstanceLock,
    on_ready: ReadyHook,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(config.listen)
        .await
        .map_err(|e| bind_error(&config, e))?;
    let addr = listener.local_addr()?;
    let url = format!("http://{addr}");
    lock.publish(&url)?;
    tracing::info!(version = mh_app::VERSION, data_dir = %dirs.root().display(), %url, "MineHost đã chạy");

    if allow_browser && config.open_browser {
        open_browser(&url);
    }

    let app = Arc::new(open_app(dirs)?);
    // Task chỉ số giữ `Weak<App>` và dừng khi `App` bị hủy lúc thoát.
    app.spawn_metrics();
    app.spawn_restart_schedule();
    app.spawn_backup_schedule();
    let lan = mh_api::LanState::new();
    let router = mh_api::router_with_lan(Arc::clone(&app), addr.port(), Arc::clone(&lan));
    let lan_manager = start_lan(addr, &app, &router, lan);
    // `ConnectInfo` cho giới hạn đăng nhập sai theo IP (spec P2, mục 6.10).
    let router = router.into_make_service_with_connect_info::<SocketAddr>();
    let (stop_http, http_stopped) = tokio::sync::oneshot::channel::<()>();
    let server = axum::serve(listener, router).with_graceful_shutdown(async {
        let _ = http_stopped.await;
    });
    let server = tokio::spawn(server.into_future());

    // Mở web trước: reconcile có thể phải chờ Java cũ nhả cổng tới vài giây.
    match app.reconcile().await {
        Ok(0) => {}
        Ok(n) => tracing::info!(n, "đã bật lại các server đang chạy lần trước"),
        Err(err) => tracing::error!(%err, "không đọc được danh sách server"),
    }
    on_ready(&app, &url);

    tokio::select! {
        () = shutdown_signal() => {}
        () = app.shutdown_requested() => tracing::info!("nhận lệnh Thoát"),
    }
    // Web vẫn chạy trong lúc tắt server, để giao diện thấy từng server dừng.
    app.shutdown_all(|done, total| tracing::info!(done, total, "đang tắt các server"))
        .await;
    if let Some(manager) = lan_manager {
        manager.stop().await;
    }
    let _ = stop_http.send(());
    match tokio::time::timeout(HTTP_DRAIN, server).await {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(err))) => tracing::warn!(%err, "máy chủ web dừng bất thường"),
        Ok(Err(err)) => tracing::warn!(%err, "task máy chủ web lỗi"),
        Err(_) => tracing::warn!("còn kết nối chưa đóng, thoát luôn"),
    }
    tracing::info!("MineHost đã tắt");
    Ok(())
}

/// LAN chỉ có nghĩa khi controller nghe trên `127.0.0.1`. Người dùng tự đổi `listen` sang
/// địa chỉ khác thì cổng đó đã mở theo ý họ; mở thêm listener cùng cổng sẽ đụng nhau.
fn start_lan(
    addr: SocketAddr,
    app: &Arc<App>,
    router: &axum::Router,
    lan: Arc<mh_api::LanState>,
) -> Option<LanManager> {
    if !addr.ip().is_loopback() {
        tracing::warn!(%addr, "`listen` không phải 127.0.0.1, bỏ qua công tắc LAN");
        return None;
    }
    Some(LanManager::spawn(
        Arc::clone(app),
        router.clone(),
        lan,
        addr.port(),
        Arc::new(mh_platform::system::lan_ipv4),
    ))
}

fn open_app(dirs: DataDirs) -> anyhow::Result<App> {
    #[cfg(feature = "e2e")]
    if let Some(java) = e2e_java_bin() {
        return e2e_app(dirs, java);
    }
    App::new(dirs).context("không khởi tạo được MineHost")
}

/// Đường dẫn trong `MINEHOST_E2E_JAVA`. Biến rỗng thì coi như không bật chế độ e2e.
#[cfg(feature = "e2e")]
fn e2e_java_bin() -> Option<std::path::PathBuf> {
    let raw = std::path::PathBuf::from(std::env::var_os(mh_app::e2e::JAVA_BIN_ENV)?);
    if raw.as_os_str().is_empty() {
        return None;
    }
    // Java chạy với thư mục làm việc là thư mục server, nên đường dẫn tương đối phải
    // được gắn vào cwd của controller trước khi spawn.
    let path = match std::env::current_dir() {
        Ok(cwd) => cwd.join(&raw),
        Err(_) => raw,
    };
    Some(path)
}

#[cfg(feature = "e2e")]
fn e2e_app(dirs: DataDirs, java: std::path::PathBuf) -> anyhow::Result<App> {
    // Bản này không tải Paper hay Java. Một dòng warn để khỏi tưởng đang chạy server thật.
    tracing::warn!(
        java = %java.display(),
        env = mh_app::e2e::JAVA_BIN_ENV,
        "chế độ e2e: không gọi mạng; Paper giả 1.21.4 và 1.20.4; Java là binary trong biến môi trường"
    );
    let deps = mh_app::e2e::deps(java);
    let app = App::with_deps(dirs, deps).context("không khởi tạo được MineHost")?;
    Ok(app.with_players(mh_app::e2e::players()))
}

pub fn open_browser(url: &str) {
    if let Err(err) = open::that_detached(url) {
        tracing::warn!(%err, "không mở được trình duyệt; hãy tự mở {url}");
    }
}

fn bind_error(config: &Config, err: io::Error) -> anyhow::Error {
    if err.kind() == io::ErrorKind::AddrInUse {
        anyhow::anyhow!(
            "[{}] cổng {} đang bị chương trình khác dùng. Đổi `listen` trong config.toml rồi chạy lại.",
            ErrorCode::PortInUse,
            config.listen.port()
        )
    } else {
        anyhow::Error::new(err).context(format!("không mở được {}", config.listen))
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::error!(%err, "không lắng nghe được Ctrl+C");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                tracing::error!(%err, "không lắng nghe được SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("nhận tín hiệu tắt, đang dừng");
}
