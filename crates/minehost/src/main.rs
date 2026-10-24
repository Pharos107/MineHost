//! Điểm vào của MineHost: đọc cấu hình, bật log, chạy API và mở trình duyệt.

// Bản release trên Windows chạy nền, không mở cửa sổ console (spec P1, mục 5.9).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod cli;
mod config;
mod instance;
mod logging;
mod serve;
#[cfg(windows)]
mod tray;
#[cfg(windows)]
mod win32;

use std::io::Write;

use anyhow::Context;
use clap::Parser;
use mh_platform::DataDirs;

use crate::cli::{Cli, Command};
use crate::instance::Acquired;

fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    win32::attach_parent_console();
    let cli = Cli::parse();
    let dirs = DataDirs::resolve(cli.data_dir.clone())?;

    match cli.command.unwrap_or_default() {
        Command::Run => run(dirs, !cli.no_browser, !cli.no_tray),
        Command::Openapi => {
            let json = mh_api::openapi()
                .to_pretty_json()
                .context("không sinh được OpenAPI")?;
            writeln!(std::io::stdout().lock(), "{json}")?;
            Ok(())
        }
        Command::Paths => {
            let mut out = std::io::stdout().lock();
            writeln!(out, "data:    {}", dirs.root().display())?;
            writeln!(out, "config:  {}", dirs.config_file().display())?;
            writeln!(out, "logs:    {}", dirs.logs().display())?;
            writeln!(out, "servers: {}", dirs.servers().display())?;
            Ok(())
        }
    }
}

fn run(dirs: DataDirs, allow_browser: bool, with_tray: bool) -> anyhow::Result<()> {
    dirs.ensure()?;
    let config = config::load_or_create(&dirs.config_file())?;
    let _log_guard = logging::init(&dirs.logs(), &config.log_level)?;

    let lock = match instance::acquire(&dirs)? {
        Acquired::First(lock) => lock,
        Acquired::Running { url } => {
            // Bấm đúp lần nữa: không chạy bản thứ hai, chỉ mở lại giao diện của bản đang chạy.
            tracing::info!(%url, "MineHost đã chạy, mở lại trình duyệt");
            if allow_browser {
                serve::open_browser(&url);
            }
            return Ok(());
        }
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("không tạo được tokio runtime")?;
    let controller = move |on_ready: serve::ReadyHook| {
        runtime.block_on(serve::run(dirs, config, allow_browser, &lock, on_ready))
    };

    #[cfg(windows)]
    if with_tray {
        return tray::run(controller);
    }
    let _ = with_tray;
    controller(Box::new(|_, _| {}))
}
