use std::io;
use std::sync::Arc;

use anyhow::Context;
use mh_app::App;
use mh_core::ErrorCode;
use mh_platform::DataDirs;
use tokio::net::TcpListener;

use crate::config::Config;

/// Chạy API và UI cho tới khi nhận Ctrl+C hoặc `SIGTERM`.
pub async fn run(dirs: DataDirs, config: Config, allow_browser: bool) -> anyhow::Result<()> {
    let listener = TcpListener::bind(config.listen)
        .await
        .map_err(|e| bind_error(&config, e))?;
    let url = format!("http://{}", listener.local_addr()?);
    tracing::info!(version = mh_app::VERSION, data_dir = %dirs.root().display(), %url, "MineHost đã chạy");

    if allow_browser
        && config.open_browser
        && let Err(err) = open::that_detached(&url)
    {
        tracing::warn!(%err, "không mở được trình duyệt; hãy tự mở {url}");
    }

    let router = mh_api::router(Arc::new(App::new(dirs)));
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("máy chủ web dừng bất thường")?;

    tracing::info!("MineHost đã tắt");
    Ok(())
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
