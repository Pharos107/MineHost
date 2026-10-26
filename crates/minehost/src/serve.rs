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

    let app = Arc::new(App::new(dirs).context("không khởi tạo được MineHost")?);
    // `ConnectInfo` cho giới hạn đăng nhập sai theo IP (spec P2, mục 6.10).
    let router = mh_api::router(Arc::clone(&app), addr.port())
        .into_make_service_with_connect_info::<SocketAddr>();
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
