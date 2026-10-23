//! Lớp hệ điều hành của MineHost.
//!
//! Mọi thứ phụ thuộc Windows/Linux nằm ở đây, để các crate phía trên không phải rẽ nhánh theo OS.

pub mod dirs;
pub mod error;
pub mod fs;
pub mod safe_path;
pub mod system;

pub use dirs::DataDirs;
pub use error::PlatformError;
pub use safe_path::{SafePath, SafeRoot};
pub use system::{DiskSpace, SystemInfo};
