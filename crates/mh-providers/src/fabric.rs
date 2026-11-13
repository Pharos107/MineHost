//! Fabric qua meta v2 (spec P3, mục 3.1). Cài bằng installer chính thức với
//! `-downloadMinecraft`, nên lần bật đầu không cần mạng.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use mh_core::instance::JarName;
use mh_core::{Launch, LoaderVersion, McVersion};
use mh_net::{Checksum, HttpClient, hosts};
use serde::Deserialize;

use crate::loader::{INSTALL_DIR, InstallPlan, InstallerRun, LoaderRelease, PlannedFile, pick, recommended_first};
use crate::{ProviderError, Release};

pub const META_BASE: &str = "https://meta.fabricmc.net/v2";
const SOURCE: &str = "Fabric";
const TTL: Duration = Duration::from_secs(60 * 60);
/// File `.sha256` cạnh jar trên maven chỉ có 64 ký tự hex, đôi khi kèm tên file.
const MAX_HASH_FILE: usize = 256;
pub const LAUNCH_JAR: &str = "fabric-server-launch.jar";

#[derive(Debug, Clone)]
pub struct Fabric {
    http: Arc<HttpClient>,
    base_url: String,
}

#[derive(Deserialize)]
struct RawGame {
    version: String,
    stable: bool,
}

#[derive(Deserialize)]
struct RawLoaderEntry {
    loader: RawVersion,
}

#[derive(Deserialize)]
struct RawVersion {
    version: String,
    stable: bool,
}

#[derive(Deserialize)]
struct RawInstaller {
    url: String,
    version: String,
    stable: bool,
}

impl Fabric {
    pub fn new(http: Arc<HttpClient>) -> Self {
        Self::with_base_url(http, META_BASE)
    }

    /// Dùng trong test để trỏ tới server giả.
    pub fn with_base_url(http: Arc<HttpClient>, base_url: impl Into<String>) -> Self {
        Self {
            http,
            base_url: base_url.into(),
        }
    }

    /// Bản release của Mojang mà Fabric hỗ trợ, giữ thứ tự của `releases`.
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

    /// Bản loader cho `mc`, bản đề xuất đứng đầu. Bỏ qua bản có tên không hợp lệ.
    pub async fn loader_versions(&self, mc: &McVersion) -> Result<Vec<LoaderRelease>, ProviderError> {
        let url = format!("{}/versions/loader/{mc}", self.base_url);
        let entries: Vec<RawLoaderEntry> = self.http.get_json(&url, TTL).await?.value;
        let releases = entries
            .into_iter()
            .filter_map(|e| {
                let version = LoaderVersion::parse(&e.loader.version).ok()?;
                Some(LoaderRelease {
                    version,
                    stable: e.loader.stable,
                })
            })
            .collect();
        Ok(recommended_first(releases))
    }

    /// Kế hoạch cài: tải installer ổn định mới nhất, chạy với `-downloadMinecraft`.
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
        let installer = self.installer().await?;
        let checksum = self.installer_sha256(&installer.url).await?;
        let jar = format!("{INSTALL_DIR}/fabric-installer-{}.jar", installer.version);
        let launch_jar = JarName::parse(LAUNCH_JAR)
            .map_err(|_| ProviderError::invalid(SOURCE, "tên jar chạy không hợp lệ"))?;
        Ok(InstallPlan {
            minecraft: mc.clone(),
            loader_version: loader.version.clone(),
            files: vec![PlannedFile {
                url: installer.url,
                checksum,
                size: None,
                dest: jar.clone(),
                hosts: hosts::LOADERS,
            }],
            installer: Some(InstallerRun {
                jar,
                args: vec![
                    "server".to_owned(),
                    "-mcversion".to_owned(),
                    mc.to_string(),
                    "-loader".to_owned(),
                    loader.version.to_string(),
                    "-downloadMinecraft".to_owned(),
                ],
            }),
            launch: Launch::Jar(launch_jar),
        })
    }

    async fn installer(&self) -> Result<RawInstaller, ProviderError> {
        let url = format!("{}/versions/installer", self.base_url);
        let all: Vec<RawInstaller> = self.http.get_json(&url, TTL).await?.value;
        all.into_iter()
            .find(|i| i.stable)
            .ok_or_else(|| ProviderError::invalid(SOURCE, "không có installer ổn định"))
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
