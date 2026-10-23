//! Use case của MineHost.
//!
//! `mh-api` và CLI chỉ gọi vào đây, không gọi thẳng các crate tầng dưới.

use mh_platform::{DataDirs, SystemInfo};

/// Phiên bản controller, lấy từ `Cargo.toml` của workspace.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Trạng thái dùng chung của controller. Bọc trong `Arc` để chia sẻ giữa các request.
#[derive(Debug)]
pub struct App {
    dirs: DataDirs,
}

impl App {
    pub fn new(dirs: DataDirs) -> Self {
        Self { dirs }
    }

    pub fn dirs(&self) -> &DataDirs {
        &self.dirs
    }

    /// Thông tin máy. Chặn luồng vài mili giây, nên gọi trong `spawn_blocking`.
    pub fn system_info(&self) -> SystemInfo {
        mh_platform::system::probe(self.dirs.root())
    }
}
