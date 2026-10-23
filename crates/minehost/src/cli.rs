use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "minehost", version, about = "Host server Minecraft dễ dàng.")]
pub struct Cli {
    /// Thư mục dữ liệu, thay cho thư mục mặc định của hệ điều hành.
    #[arg(long, global = true, env = "MINEHOST_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    /// Không tự mở trình duyệt khi khởi động.
    #[arg(long, global = true)]
    pub no_browser: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Default, Subcommand)]
pub enum Command {
    /// Chạy controller và giao diện web (mặc định).
    #[default]
    Run,
    /// In spec OpenAPI ra stdout, dùng để sinh client cho `ui/`.
    Openapi,
    /// In các thư mục dữ liệu đang dùng.
    Paths,
}
