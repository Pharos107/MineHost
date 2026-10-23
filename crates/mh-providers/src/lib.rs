//! Nguồn server: danh sách phiên bản, file cần tải và cách chạy.
//!
//! P1 có Mojang (phiên bản và Java cần dùng) và Paper. Fabric, Forge, NeoForge, Modrinth,
//! CurseForge thêm từ P3.

mod error;
pub mod mojang;

pub use error::ProviderError;
pub use mojang::{Mojang, Release, ServerJar, VersionDetail};
