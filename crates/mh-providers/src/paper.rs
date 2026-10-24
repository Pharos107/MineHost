//! Paper qua Fill API v3 (spec P1, mục 5.3).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use mh_core::McVersion;
use mh_core::instance::JarName;
use mh_net::{Checksum, HttpClient};
use serde::Deserialize;

use crate::{ProviderError, Release, ServerJar};

pub const API_BASE: &str = "https://fill.papermc.io/v3/projects/paper";
const SOURCE: &str = "Paper";
const PROJECT_TTL: Duration = Duration::from_secs(60 * 60);
/// Paper ra build mới nhiều lần mỗi ngày; 10 phút đủ để wizard không gọi lại liên tục.
const BUILDS_TTL: Duration = Duration::from_secs(10 * 60);
const SERVER_DOWNLOAD: &str = "server:default";

#[derive(Debug, Clone)]
pub struct Paper {
    http: Arc<HttpClient>,
    base_url: String,
}

/// Build Paper sẽ cài cho một phiên bản Minecraft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperBuild {
    pub version: McVersion,
    pub build: u32,
    pub jar: ServerJar,
}

/// Cách chạy server sau khi đã có jar. `mh-runtime` chỉ nhận jar và tham số, không biết
/// jar đó từ đâu ra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchRecipe {
    pub jar: JarName,
    pub args: Vec<String>,
}

#[derive(Deserialize)]
struct RawProject {
    /// Nhóm phiên bản, ví dụ `"1.21": ["1.21.11", "1.21.11-rc3", …]`.
    versions: HashMap<String, Vec<String>>,
}

#[derive(Deserialize)]
struct RawBuild {
    id: u32,
    channel: String,
    #[serde(default)]
    downloads: HashMap<String, RawDownload>,
}

#[derive(Deserialize)]
struct RawDownload {
    checksums: RawChecksums,
    size: u64,
    url: String,
}

#[derive(Deserialize)]
struct RawChecksums {
    sha256: String,
}

impl Paper {
    pub fn new(http: Arc<HttpClient>) -> Self {
        Self::with_base_url(http, API_BASE)
    }

    /// Dùng trong test để trỏ tới server giả.
    pub fn with_base_url(http: Arc<HttpClient>, base_url: impl Into<String>) -> Self {
        Self {
            http,
            base_url: base_url.into(),
        }
    }

    /// Các bản release của Mojang mà Paper có, giữ thứ tự của `releases` (mới nhất trước).
    ///
    /// Lọc theo danh sách của Mojang thay vì đoán từ tên: Paper liệt kê cả `-rc`, `-pre`.
    pub async fn versions(&self, releases: &[Release]) -> Result<Vec<Release>, ProviderError> {
        let project: RawProject = self.http.get_json(&self.base_url, PROJECT_TTL).await?.value;
        let known: HashSet<String> = project.versions.into_values().flatten().collect();
        Ok(releases
            .iter()
            .filter(|r| known.contains(r.id.as_str()))
            .cloned()
            .collect())
    }

    /// Build mới nhất có `channel = "STABLE"`. Không có build nào ổn định thì báo không có phiên bản.
    pub async fn latest_stable_build(
        &self,
        version: &McVersion,
    ) -> Result<PaperBuild, ProviderError> {
        let url = format!("{}/versions/{version}/builds", self.base_url);
        let builds: Vec<RawBuild> = self.http.get_json(&url, BUILDS_TTL).await?.value;
        let build = builds
            .into_iter()
            .filter(|b| b.channel == "STABLE")
            .max_by_key(|b| b.id)
            .ok_or_else(|| ProviderError::VersionNotFound {
                source_name: SOURCE,
                version: version.to_string(),
            })?;
        let mut downloads = build.downloads;
        let raw = downloads
            .remove(SERVER_DOWNLOAD)
            .ok_or_else(|| ProviderError::invalid(SOURCE, "build không có server:default"))?;
        let checksum = Checksum::sha256(&raw.checksums.sha256)
            .ok_or_else(|| ProviderError::invalid(SOURCE, "sha256 sai định dạng"))?;
        Ok(PaperBuild {
            version: version.clone(),
            build: build.id,
            jar: ServerJar {
                url: raw.url,
                checksum,
                size: raw.size,
            },
        })
    }

    /// Paper chạy thẳng jar tải về; `--nogui` để không mở cửa sổ Swing trên máy có màn hình.
    pub fn recipe() -> LaunchRecipe {
        LaunchRecipe {
            jar: JarName::server_jar(),
            args: vec!["--nogui".to_owned()],
        }
    }
}
