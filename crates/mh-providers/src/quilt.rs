//! Quilt qua meta v3 (spec P3, mục 3.1). Meta trả danh sách loader không theo thứ tự nào, nên
//! MineHost tự sắp theo số phiên bản; bản không có hậu tố `-beta…` được coi là ổn định.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use mh_core::instance::JarName;
use mh_core::{Launch, LoaderVersion, McVersion};
use mh_net::{Checksum, HostAllowlist, HttpClient, hosts};
use serde::Deserialize;

use crate::loader::{
    InstallPlan, InstallerRun, LoaderRelease, PlannedFile, check_loader_url, compare_versions,
    installer_path, pick, recommended_first,
};
use crate::{ProviderError, Release};

pub const META_BASE: &str = "https://meta.quiltmc.org/v3";
const SOURCE: &str = "Quilt";
const TTL: Duration = Duration::from_secs(60 * 60);
pub const LAUNCH_JAR: &str = "quilt-server-launch.jar";

#[derive(Debug, Clone)]
pub struct Quilt {
    http: Arc<HttpClient>,
    base_url: String,
    hosts: HostAllowlist,
}

#[derive(Deserialize)]
struct RawGame {
    version: String,
    stable: bool,
}

#[derive(Deserialize)]
struct RawLoaderEntry {
    loader: RawLoader,
}

#[derive(Deserialize)]
struct RawLoader {
    version: String,
}

#[derive(Deserialize)]
struct RawInstaller {
    url: String,
    version: String,
    file_size: Option<u64>,
    hashes: RawHashes,
}

#[derive(Deserialize)]
struct RawHashes {
    sha256: String,
}

impl Quilt {
    pub fn new(http: Arc<HttpClient>) -> Self {
        Self::with_base_url(http, META_BASE)
    }

    /// Dùng trong test để trỏ tới server giả.
    pub fn with_base_url(http: Arc<HttpClient>, base_url: impl Into<String>) -> Self {
        Self {
            http,
            base_url: base_url.into(),
            hosts: hosts::LOADERS,
        }
    }

    /// Dùng trong test: cho installer nằm trên server giả.
    pub fn with_hosts(mut self, hosts: HostAllowlist) -> Self {
        self.hosts = hosts;
        self
    }

    /// Bản release của Mojang mà Quilt hỗ trợ, giữ thứ tự của `releases`.
    pub async fn versions(&self, releases: &[Release]) -> Result<Vec<Release>, ProviderError> {
        let url = format!("{}/versions/game", self.base_url);
        let games: Vec<RawGame> = self.http.get_json(&url, TTL).await?.value;
        let known: HashSet<String> = games
            .into_iter()
            .filter(|g| g.stable)
            .map(|g| g.version)
            .collect();
        Ok(releases
            .iter()
            .filter(|r| known.contains(r.id.as_str()))
            .cloned()
            .collect())
    }

    /// Bản loader cho `mc`, mới nhất trước, bản ổn định mới nhất đứng đầu.
    pub async fn loader_versions(
        &self,
        mc: &McVersion,
    ) -> Result<Vec<LoaderRelease>, ProviderError> {
        let url = format!("{}/versions/loader/{mc}", self.base_url);
        let entries: Vec<RawLoaderEntry> = self.http.get_json(&url, TTL).await?.value;
        let mut releases: Vec<LoaderRelease> = entries
            .into_iter()
            .filter_map(|e| {
                let version = LoaderVersion::parse(&e.loader.version).ok()?;
                let stable = !version.as_str().contains('-');
                Some(LoaderRelease { version, stable })
            })
            .collect();
        releases.sort_by(|a, b| compare_versions(b.version.as_str(), a.version.as_str()));
        Ok(recommended_first(releases))
    }

    /// Kế hoạch cài: installer mới nhất, `install server … --download-server`.
    pub async fn plan(
        &self,
        mc: &McVersion,
        wanted: Option<&LoaderVersion>,
    ) -> Result<InstallPlan, ProviderError> {
        let releases = self.loader_versions(mc).await?;
        let loader = pick(&releases, wanted).ok_or_else(|| ProviderError::VersionNotFound {
            source_name: SOURCE,
            version: wanted.map_or_else(|| mc.to_string(), ToString::to_string),
        })?;
        let url = format!("{}/versions/installer", self.base_url);
        let installers: Vec<RawInstaller> = self.http.get_json(&url, TTL).await?.value;
        let installer = installers
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::invalid(SOURCE, "không có installer"))?;
        let checksum = Checksum::sha256(&installer.hashes.sha256)
            .ok_or_else(|| ProviderError::invalid(SOURCE, "sha256 của installer sai định dạng"))?;
        let jar = installer_path(SOURCE, "quilt-installer", &installer.version)?;
        check_loader_url(SOURCE, &self.hosts, &installer.url)?;
        let launch_jar = JarName::parse(LAUNCH_JAR)
            .map_err(|_| ProviderError::invalid(SOURCE, "tên jar chạy không hợp lệ"))?;
        Ok(InstallPlan {
            minecraft: mc.clone(),
            loader_version: loader.version.clone(),
            files: vec![PlannedFile {
                url: installer.url,
                checksum,
                size: installer.file_size,
                dest: jar.clone(),
                hosts: self.hosts,
            }],
            installer: Some(InstallerRun {
                jar,
                args: vec![
                    "install".to_owned(),
                    "server".to_owned(),
                    mc.to_string(),
                    loader.version.to_string(),
                    "--download-server".to_owned(),
                    "--install-dir=.".to_owned(),
                ],
            }),
            launch: Launch::Jar(launch_jar),
        })
    }
}
