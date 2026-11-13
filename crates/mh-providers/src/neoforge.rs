//! NeoForge qua maven (spec P3, mục 3.1). Chỉ tọa độ `net.neoforged:neoforge`, tức từ
//! Minecraft 1.20.2; NeoForge 1.20.1 (`net.neoforged:forge`) không hỗ trợ (spec P3, mục 1).
//!
//! Cài bằng `neoforge-<v>-installer.jar --installServer`, chạy bằng file tham số trong
//! `libraries/net/neoforged/neoforge/<v>/`.

use std::collections::HashSet;
use std::sync::Arc;

use mh_core::{ArgsFilePath, Launch, LoaderVersion, McVersion};
use mh_net::{Checksum, HostAllowlist, HttpClient, hosts};

use crate::loader::{
    InstallPlan, InstallerRun, LoaderRelease, PlannedFile, check_loader_url, compare_versions,
    installer_path, maven_versions, pick, recommended_first,
};
use crate::{ProviderError, Release};

pub const MAVEN_BASE: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge";
const SOURCE: &str = "NeoForge";
const MAX_METADATA: usize = 8 * 1024 * 1024;
const MAX_HASH_FILE: usize = 256;

#[derive(Debug, Clone)]
pub struct NeoForge {
    http: Arc<HttpClient>,
    maven_base: String,
    hosts: HostAllowlist,
}

/// Bản Minecraft mà một bản NeoForge nhắm tới.
///
/// Trước 26.1: ba phần, hai phần đầu là số phụ và số vá của Minecraft (`21.4.140` là 1.21.4,
/// `21.0.167` là 1.21). Từ 26.1: bốn phần, ba phần đầu là Minecraft (`26.1.2.76` là 26.1.2,
/// `26.3.0.103` là 26.3). Bản dựng cho snapshot (`+snapshot…`, `alpha`) và bản đùa ngày 1/4 không
/// tương ứng bản release nào nên trả `None`.
pub(crate) fn minecraft_of(version: &str) -> Option<String> {
    if version.contains('+') || version.contains("alpha") {
        return None;
    }
    let core = version.split('-').next()?;
    let parts: Vec<u32> = core
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    match parts.as_slice() {
        [major @ 20..=25, minor, _] if (*major, *minor) >= (20, 2) => Some(match minor {
            0 => format!("1.{major}"),
            _ => format!("1.{major}.{minor}"),
        }),
        [year @ 26.., drop, 0, _] => Some(format!("{year}.{drop}")),
        [year @ 26.., drop, hotfix, _] => Some(format!("{year}.{drop}.{hotfix}")),
        _ => None,
    }
}

impl NeoForge {
    pub fn new(http: Arc<HttpClient>) -> Self {
        Self::with_base_url(http, MAVEN_BASE)
    }

    /// Dùng trong test để trỏ tới server giả.
    pub fn with_base_url(http: Arc<HttpClient>, maven_base: impl Into<String>) -> Self {
        Self {
            http,
            maven_base: maven_base.into(),
            hosts: hosts::LOADERS,
        }
    }

    /// Dùng trong test: cho installer nằm trên server giả.
    pub fn with_hosts(mut self, hosts: HostAllowlist) -> Self {
        self.hosts = hosts;
        self
    }

    /// Bản release của Mojang có NeoForge, giữ thứ tự của `releases`.
    pub async fn versions(&self, releases: &[Release]) -> Result<Vec<Release>, ProviderError> {
        let all = self.all_versions().await?;
        let known: HashSet<String> = all.iter().filter_map(|v| minecraft_of(v)).collect();
        Ok(releases
            .iter()
            .filter(|r| known.contains(r.id.as_str()))
            .cloned()
            .collect())
    }

    /// Bản NeoForge cho `mc`, mới nhất trước. Bản `-beta` chỉ có mặt khi `mc` chưa có bản ổn định
    /// nào (NeoForge cho 26.1 và 26.1.1 chưa từng ra khỏi beta).
    pub async fn loader_versions(&self, mc: &McVersion) -> Result<Vec<LoaderRelease>, ProviderError> {
        let all = self.all_versions().await?;
        let mut releases: Vec<LoaderRelease> = all
            .iter()
            .filter(|v| minecraft_of(v).as_deref() == Some(mc.as_str()))
            .filter_map(|v| LoaderVersion::parse(v).ok())
            .map(|version| LoaderRelease {
                stable: !version.as_str().contains('-'),
                version,
            })
            .collect();
        if releases.iter().any(|r| r.stable) {
            releases.retain(|r| r.stable);
        }
        releases.sort_by(|a, b| compare_versions(b.version.as_str(), a.version.as_str()));
        Ok(recommended_first(releases))
    }

    pub async fn plan(
        &self,
        mc: &McVersion,
        wanted: Option<&LoaderVersion>,
    ) -> Result<InstallPlan, ProviderError> {
        let releases = self.loader_versions(mc).await?;
        let loader = pick(&releases, wanted)
            .ok_or_else(|| ProviderError::VersionNotFound {
                source_name: SOURCE,
                version: wanted.map_or_else(|| mc.to_string(), ToString::to_string),
            })?
            .version
            .clone();
        let url = format!("{0}/{loader}/neoforge-{loader}-installer.jar", self.maven_base);
        check_loader_url(SOURCE, &self.hosts, &url)?;
        let checksum = self.installer_sha256(&url).await?;
        let jar = installer_path(SOURCE, "neoforge-installer", loader.as_str())?;
        let args = format!("libraries/net/neoforged/neoforge/{loader}/{{os}}_args.txt");
        let args = ArgsFilePath::parse(&args)
            .map_err(|_| ProviderError::invalid(SOURCE, "đường dẫn file tham số không hợp lệ"))?;
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
            launch: Launch::ArgsFile(args),
        })
    }

    async fn all_versions(&self) -> Result<Vec<String>, ProviderError> {
        let url = format!("{}/maven-metadata.xml", self.maven_base);
        let body = self.http.get_bytes(&url, MAX_METADATA).await?;
        let xml = String::from_utf8_lossy(&body);
        Ok(maven_versions(&xml).into_iter().map(str::to_owned).collect())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_both_numbering_schemes() {
        let cases = [
            ("20.2.86", Some("1.20.2")),
            ("20.4.0-beta", Some("1.20.4")),
            ("21.0.167", Some("1.21")),
            ("21.4.140", Some("1.21.4")),
            ("21.11.3", Some("1.21.11")),
            ("26.1.0.19-beta", Some("26.1")),
            ("26.1.2.76", Some("26.1.2")),
            ("26.3.0.103", Some("26.3")),
            ("20.1.5", None),
            ("26.1.0.0-alpha.1+snapshot-1", None),
            ("0.25w14craftmine", None),
            ("21.4", None),
        ];
        for (neo, mc) in cases {
            assert_eq!(minecraft_of(neo).as_deref(), mc, "{neo}");
        }
    }
}
