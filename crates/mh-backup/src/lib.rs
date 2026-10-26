//! Nén và giải nén backup `tar.zst` (spec P2, mục 5.6).
//!
//! Mọi hàm công khai đều đồng bộ. `mh-app` gọi chúng trong `spawn_blocking`
//! để không chiếm runtime async.

mod create;
mod error;
mod io_util;
mod restore;

pub use create::{DEFAULT_EXCLUDE, create};
pub use error::BackupError;
pub use restore::{restore, unpack_tar};

/// File `.tar.zst` đã ghi xong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveInfo {
    /// Số byte đã ghi ra file nén.
    pub size: u64,
    /// SHA-256 hex chữ thường của đúng những byte đó.
    pub sha256: String,
}
