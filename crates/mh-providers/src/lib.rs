//! Nguồn server: danh sách phiên bản, file cần tải và cách chạy.
//!
//! P1 có Mojang (phiên bản, Java cần dùng, jar Vanilla) và Paper; P2 thêm Purpur. P3 thêm mod
//! loader (Fabric, Quilt, rồi Forge, NeoForge), sau đó Modrinth, CurseForge.

pub mod curseforge;
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

pub use curseforge::CurseForge;
pub use error::ProviderError;
pub use fabric::Fabric;
pub use forge::Forge;
pub use loader::{InstallPlan, InstallerRun, LoaderRelease, PlannedFile};
pub use modpack::{
    BlockedPackFile, PackFile, PackLimits, PackPlan, extract_mrpack_overrides,
    extract_mrpack_overrides_with, plan_mrpack, plan_mrpack_with, read_mrpack, read_mrpack_with,
};
pub use modrinth::Modrinth;
pub use mojang::{Mojang, Profile, Release, ServerJar, VersionDetail};
pub use neoforge::NeoForge;
pub use paper::{LaunchRecipe, Paper, PaperBuild};
pub use purpur::{Purpur, PurpurBuild};
pub use quilt::Quilt;

use serde::Deserialize;
use url::Url;

pub(crate) fn endpoint(
    source: &'static str,
    base: &str,
    segments: &[&str],
) -> Result<Url, ProviderError> {
    let mut url =
        Url::parse(base).map_err(|_| ProviderError::invalid(source, "base URL không hợp lệ"))?;
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|()| ProviderError::invalid(source, "base URL không hỗ trợ path"))?;
        path.pop_if_empty();
        for seg in segments {
            path.push(seg);
        }
    }
    Ok(url)
}

pub(crate) fn non_empty_opt<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<String>, D::Error> {
    let opt = Option::<String>::deserialize(d)?;
    Ok(opt.filter(|s| !s.trim().is_empty()))
}
