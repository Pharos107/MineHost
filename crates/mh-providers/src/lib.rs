//! Nguồn server: danh sách phiên bản, file cần tải và cách chạy.
//!
//! P1 có Mojang (phiên bản, Java cần dùng, jar Vanilla) và Paper; P2 thêm Purpur. Fabric, Forge,
//! NeoForge, Modrinth, CurseForge thêm từ P3.

mod error;
pub mod mojang;
pub mod paper;
pub mod purpur;

pub use error::ProviderError;
pub use mojang::{Mojang, Profile, Release, ServerJar, VersionDetail};
pub use paper::{LaunchRecipe, Paper, PaperBuild};
pub use purpur::{Purpur, PurpurBuild};
