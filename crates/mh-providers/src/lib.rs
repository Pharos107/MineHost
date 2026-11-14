//! Nguồn server: danh sách phiên bản, file cần tải và cách chạy.
//!
//! P1 có Mojang (phiên bản, Java cần dùng, jar Vanilla) và Paper; P2 thêm Purpur. P3 thêm mod
//! loader (Fabric, Quilt, rồi Forge, NeoForge), sau đó Modrinth, CurseForge.

mod error;
pub mod fabric;
pub mod forge;
pub mod loader;
pub mod modpack;
pub mod modrinth;
pub mod mojang;
pub mod neoforge;
pub mod paper;
pub mod purpur;
pub mod quilt;

pub use error::ProviderError;
pub use fabric::Fabric;
pub use forge::Forge;
pub use loader::{InstallPlan, InstallerRun, LoaderRelease, PlannedFile};
pub use modpack::{BlockedPackFile, PackFile, PackLimits, PackPlan, read_mrpack, read_mrpack_with};
pub use modrinth::Modrinth;
pub use mojang::{Mojang, Profile, Release, ServerJar, VersionDetail};
pub use neoforge::NeoForge;
pub use paper::{LaunchRecipe, Paper, PaperBuild};
pub use purpur::{Purpur, PurpurBuild};
pub use quilt::Quilt;
