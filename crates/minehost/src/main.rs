//! Điểm vào của MineHost: đọc cấu hình, bật log, chạy API và mở trình duyệt.

mod cli;
mod config;
mod logging;
mod serve;

use std::io::Write;

use anyhow::Context;
use clap::Parser;
use mh_platform::DataDirs;

use crate::cli::{Cli, Command};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let dirs = DataDirs::resolve(cli.data_dir.clone())?;

    match cli.command.unwrap_or_default() {
        Command::Run => {
            dirs.ensure()?;
            let config = config::load_or_create(&dirs.config_file())?;
            let _log_guard = logging::init(&dirs.logs(), &config.log_level)?;
            serve::run(dirs, config, !cli.no_browser).await
        }
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
