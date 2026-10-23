use std::path::Path;

use anyhow::Context;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::MakeWriterExt;

/// Ghi log ra stderr và file `logs/minehost.log` (xoay vòng theo ngày).
///
/// Giữ `WorkerGuard` sống tới cuối `main`, nếu không các dòng log cuối sẽ bị mất.
/// Biến môi trường `RUST_LOG` được ưu tiên hơn `log_level` trong `config.toml`.
pub fn init(logs_dir: &Path, level: &str) -> anyhow::Result<WorkerGuard> {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(level))
        .with_context(|| format!("mức log `{level}` không hợp lệ"))?;

    let file = tracing_appender::rolling::daily(logs_dir, "minehost.log");
    let (file, guard) = tracing_appender::non_blocking(file);

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr.and(file))
        .with_ansi(false)
        .try_init()
        .map_err(|e| anyhow::anyhow!("không bật được log: {e}"))?;

    Ok(guard)
}
