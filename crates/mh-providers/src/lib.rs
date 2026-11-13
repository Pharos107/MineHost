//! Nguồn server: danh sách phiên bản, file cần tải và cách chạy.
//!
//! P1 có Mojang (phiên bản, Java cần dùng, jar Vanilla) và Paper; P2 thêm Purpur. P3 thêm mod
//! loader (Fabric, Quilt, rồi Forge, NeoForge), sau đó Modrinth, CurseForge.

mod error;
pub mod fabric;
pub mod forge;
pub mod loader;
pub mod mojang;
pub mod paper;
pub mod purpur;
pub mod quilt;

pub use error::ProviderError;
pub use fabric::Fabric;
pub use forge::Forge;
pub use loader::{InstallPlan, InstallerRun, LoaderRelease, PlannedFile};
pub use mojang::{Mojang, Profile, Release, ServerJar, VersionDetail};
pub use paper::{LaunchRecipe, Paper, PaperBuild};
pub use purpur::{Purpur, PurpurBuild};
pub use quilt::Quilt;
