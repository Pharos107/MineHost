//! Forge qua maven và `promotions_slim.json` (spec P3, mục 3.1).
//!
//! Cài bằng `forge-<mc>-<v>-installer.jar --installServer`. Dưới 1.17 installer tạo
//! `forge-<mc>-<v>.jar` chạy bằng `-jar`; từ 1.17 server chạy bằng file tham số trong `libraries/`.
//! Không hỗ trợ Minecraft cũ hơn 1.12.2 (spec P3, mục 1).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use mh_core::instance::JarName;
use mh_core::{ArgsFilePath, Launch, LoaderVersion, McVersion};
use mh_net::{Checksum, HostAllowlist, HttpClient, hosts};
use serde::Deserialize;

use crate::loader::{
    InstallPlan, InstallerRun, LoaderRelease, PlannedFile, check_loader_url, compare_versions,
    installer_path, maven_versions, pick, recommended_first,
};
use crate::{ProviderError, Release};

pub const MAVEN_BASE: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";
pub const PROMOTIONS_URL: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const SOURCE: &str = "Forge";
const TTL: Duration = Duration::from_secs(60 * 60);
/// `maven-metadata.xml` của Forge khoảng 200 KB; chừa rộng cho vài năm nữa.
const MAX_METADATA: usize = 8 * 1024 * 1024;
const MAX_HASH_FILE: usize = 256;

#[derive(Debug, Clone)]
pub struct Forge {
    http: Arc<HttpClient>,
    maven_base: String,
    promotions_url: String,
    hosts: HostAllowlist,
}

#[derive(Deserialize)]
struct RawPromotions {
    promos: HashMap<String, String>,
}

/// Forge dùng cách chạy nào cho bản Minecraft này.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Era {
    /// 1.12.2 tới 1.16.5: `-jar forge-<mc>-<v>.jar`.
    LegacyJar,
    /// Từ 1.17: `@libraries/net/minecraftforge/forge/<mc>-<v>/{os}_args.txt`.
    ArgsFile,
}

/// Quy tắc riêng của Forge, không phải cách so phiên bản chung (spec P1, mục 3.5): mọi bản
/// Forge cũ đều đặt tên `1.<minor>[.<patch>]`, bản từ 26.1 theo năm thì luôn dùng file tham số.
fn era(mc: &McVersion) -> Option<Era> {
    let Some(rest) = mc.as_str().strip_prefix("1.") else {
        return Some(Era::ArgsFile);
    };
    let mut parts = rest.split('.');
    let minor: u32 = parts.next()?.parse().ok()?;
    let patch: u32 = parts.next().map_or(Some(0), |p| p.parse().ok())?;
    match (minor, patch) {
        (m, _) if m < 12 => None,
        (12, p) if p < 2 => None,
        (m, _) if m < 17 => Some(Era::LegacyJar),
        _ => Some(Era::ArgsFile),
    }
}

impl Forge {
    pub fn new(http: Arc<HttpClient>) -> Self {
        Self::with_urls(http, MAVEN_BASE, PROMOTIONS_URL)
    }

    /// Dùng trong test để trỏ tới server giả.
    pub fn with_urls(
        http: Arc<HttpClient>,
        maven_base: impl Into<String>,
        promotions_url: impl Into<String>,
    ) -> Self {
        Self {
            http,
            maven_base: maven_base.into(),
            promotions_url: promotions_url.into(),
            hosts: hosts::LOADERS,
        }
    }

    /// Dùng trong test: cho installer nằm trên server giả.
    pub fn with_hosts(mut self, hosts: HostAllowlist) -> Self {
        self.hosts = hosts;
        self
    }

    /// Bản release của Mojang có Forge (từ 1.12.2), giữ thứ tự của `releases`.
    pub async fn versions(&self, releases: &[Release]) -> Result<Vec<Release>, ProviderError> {
        let promos = self.promotions().await?;
        let known: HashSet<&str> = promos
            .keys()
            .filter_map(|k| k.rsplit_once('-').map(|(mc, _)| mc))
            .collect();
        Ok(releases
            .iter()
            .filter(|r| known.contains(r.id.as_str()) && era(&r.id).is_some())
            .cloned()
            .collect())
    }

    /// Bản Forge cho `mc`, mới nhất trước; bản `recommended` (không có thì `latest`) đứng đầu.
    pub async fn loader_versions(&self, mc: &McVersion) -> Result<Vec<LoaderRelease>, ProviderError> {
        let url = format!("{}/maven-metadata.xml", self.maven_base);
        let body = self.http.get_bytes(&url, MAX_METADATA).await?;
        let xml = String::from_utf8_lossy(&body);
        let promos = self.promotions().await?;
        let promoted = promos
            .get(&format!("{mc}-recommended"))
            .or_else(|| promos.get(&format!("{mc}-latest")));
        let prefix = format!("{mc}-");
        let mut releases: Vec<LoaderRelease> = maven_versions(&xml)
            .into_iter()
            .filter_map(|full| full.strip_prefix(prefix.as_str()))
            .filter_map(|v| LoaderVersion::parse(v).ok())
            .map(|version| LoaderRelease {
                stable: promoted.is_some_and(|p| p == version.as_str()),
                version,
            })
            .collect();
        releases.sort_by(|a, b| compare_versions(b.version.as_str(), a.version.as_str()));
        Ok(recommended_first(releases))
    }

    pub async fn plan(
        &self,
        mc: &McVersion,
        wanted: Option<&LoaderVersion>,
    ) -> Result<InstallPlan, ProviderError> {
        let not_found = || ProviderError::VersionNotFound {
            source_name: SOURCE,
            version: wanted.map_or_else(|| mc.to_string(), |v| format!("{mc}-{v}")),
        };
        let era = era(mc).ok_or_else(not_found)?;
        let releases = self.loader_versions(mc).await?;
        let loader = pick(&releases, wanted).ok_or_else(not_found)?.version.clone();
        let full = format!("{mc}-{loader}");
        let url = format!("{0}/{full}/forge-{full}-installer.jar", self.maven_base);
        check_loader_url(SOURCE, &self.hosts, &url)?;
        let checksum = self.installer_sha256(&url).await?;
        let jar = installer_path(SOURCE, "forge-installer", &full)?;
        Ok(InstallPlan {
            minecraft: mc.clone(),
            loader_version: loader,
            files: vec![PlannedFile {
                url,
                checksum,
                size: None,
                dest: jar.clone(),
                hosts: self.hosts,
            }],
            installer: Some(InstallerRun {
                jar,
                args: vec!["--installServer".to_owned()],
            }),
            launch: launch_for(era, &full)?,
        })
    }

    async fn promotions(&self) -> Result<HashMap<String, String>, ProviderError> {
        let raw: RawPromotions = self.http.get_json(&self.promotions_url, TTL).await?.value;
        Ok(raw.promos)
    }

    async fn installer_sha256(&self, jar_url: &str) -> Result<Checksum, ProviderError> {
        let body = self
            .http
            .get_bytes(&format!("{jar_url}.sha256"), MAX_HASH_FILE)
            .await?;
        let text = String::from_utf8_lossy(&body);
        text.split_whitespace()
            .next()
            .and_then(Checksum::sha256)
            .ok_or_else(|| ProviderError::invalid(SOURCE, "file .sha256 của installer sai định dạng"))
    }
}

fn launch_for(era: Era, full: &str) -> Result<Launch, ProviderError> {
    let bad = || ProviderError::invalid(SOURCE, "tên file chạy server không hợp lệ");
    match era {
        Era::LegacyJar => {
            let jar = JarName::parse(&format!("forge-{full}.jar")).map_err(|_| bad())?;
            Ok(Launch::Jar(jar))
        }
        Era::ArgsFile => {
            let path = format!("libraries/net/minecraftforge/forge/{full}/{{os}}_args.txt");
            Ok(Launch::ArgsFile(ArgsFilePath::parse(&path).map_err(|_| bad())?))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn era_of(v: &str) -> Option<Era> {
        era(&McVersion::parse(v).unwrap())
    }

    #[test]
    fn era_by_minecraft_version() {
        assert_eq!(era_of("1.7.10"), None);
        assert_eq!(era_of("1.12"), None);
        assert_eq!(era_of("1.12.1"), None);
        assert_eq!(era_of("1.12.2"), Some(Era::LegacyJar));
        assert_eq!(era_of("1.16.5"), Some(Era::LegacyJar));
        assert_eq!(era_of("1.17"), Some(Era::ArgsFile));
        assert_eq!(era_of("1.21.4"), Some(Era::ArgsFile));
        assert_eq!(era_of("26.3"), Some(Era::ArgsFile));
        assert_eq!(era_of("1.x"), None);
    }
}
