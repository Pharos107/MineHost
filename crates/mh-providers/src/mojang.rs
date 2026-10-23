//! Manifest phiên bản của Mojang (spec P1, mục 5.3).

use std::sync::Arc;
use std::time::Duration;

use mh_core::McVersion;
use mh_net::{Checksum, Fetched, HttpClient};
use serde::Deserialize;
use time::OffsetDateTime;

use crate::ProviderError;

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const SOURCE: &str = "Mojang";
/// Manifest chỉ đổi khi Mojang ra bản mới; một giờ là đủ mới cho wizard.
const MANIFEST_TTL: Duration = Duration::from_secs(60 * 60);
/// JSON của từng phiên bản không bao giờ đổi, vì URL chứa sha1 của chính nó.
const DETAIL_TTL: Duration = Duration::from_secs(24 * 60 * 60);
/// Bản rất cũ không ghi `javaVersion`; chúng chạy được trên Java 8.
const LEGACY_JAVA_MAJOR: u16 = 8;

#[derive(Debug, Clone)]
pub struct Mojang {
    http: Arc<HttpClient>,
    manifest_url: String,
}

/// Một bản release, kèm thời điểm phát hành để sắp xếp (spec P1, mục 3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub id: McVersion,
    pub release_time: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionDetail {
    pub id: McVersion,
    pub java_major: u16,
    /// Jar server vanilla. Vài bản rất cũ không có.
    pub server: Option<ServerJar>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerJar {
    pub url: String,
    pub checksum: Checksum,
    pub size: u64,
}

#[derive(Deserialize)]
struct Manifest {
    versions: Vec<RawEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEntry {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    url: String,
    sha1: String,
    #[serde(with = "time::serde::rfc3339")]
    release_time: OffsetDateTime,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDetail {
    java_version: Option<RawJava>,
    #[serde(default)]
    downloads: RawDownloads,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawJava {
    major_version: u16,
}

#[derive(Default, Deserialize)]
struct RawDownloads {
    server: Option<RawFile>,
}

#[derive(Deserialize)]
struct RawFile {
    sha1: String,
    size: u64,
    url: String,
}

impl Mojang {
    pub fn new(http: Arc<HttpClient>) -> Self {
        Self::with_manifest_url(http, MANIFEST_URL)
    }

    /// Dùng trong test để trỏ tới server giả.
    pub fn with_manifest_url(http: Arc<HttpClient>, manifest_url: impl Into<String>) -> Self {
        Self {
            http,
            manifest_url: manifest_url.into(),
        }
    }

    /// Các bản `release`, mới nhất trước.
    pub async fn releases(&self) -> Result<Vec<Release>, ProviderError> {
        let manifest = self.manifest().await?;
        let mut releases: Vec<Release> = manifest
            .versions
            .into_iter()
            .filter(|v| v.kind == "release")
            .filter_map(|v| match McVersion::parse(&v.id) {
                Ok(id) => Some(Release {
                    id,
                    release_time: v.release_time,
                }),
                Err(err) => {
                    tracing::debug!(id = v.id, %err, "bỏ qua phiên bản có tên lạ");
                    None
                }
            })
            .collect();
        releases.sort_by_key(|r| std::cmp::Reverse(r.release_time));
        Ok(releases)
    }

    /// Java cần dùng và jar server của một phiên bản.
    pub async fn version_detail(&self, id: &McVersion) -> Result<VersionDetail, ProviderError> {
        let manifest = self.manifest().await?;
        let entry = manifest
            .versions
            .into_iter()
            .find(|v| v.id == id.as_str())
            .ok_or_else(|| ProviderError::VersionNotFound {
                source_name: SOURCE,
                version: id.to_string(),
            })?;
        let checksum = Checksum::sha1(&entry.sha1)
            .ok_or_else(|| ProviderError::invalid(SOURCE, "sha1 trong manifest sai định dạng"))?;
        let detail: RawDetail = self
            .http
            .get_json_checked(&entry.url, DETAIL_TTL, &checksum)
            .await?
            .value;
        let server = detail.downloads.server.map(server_jar).transpose()?;
        Ok(VersionDetail {
            id: id.clone(),
            java_major: detail
                .java_version
                .map_or(LEGACY_JAVA_MAJOR, |j| j.major_version),
            server,
        })
    }

    async fn manifest(&self) -> Result<Manifest, ProviderError> {
        let Fetched { value, stale } = self.http.get_json(&self.manifest_url, MANIFEST_TTL).await?;
        if stale {
            tracing::warn!("dùng manifest Mojang lưu lần trước, có thể thiếu bản mới nhất");
        }
        Ok(value)
    }
}

fn server_jar(raw: RawFile) -> Result<ServerJar, ProviderError> {
    let checksum = Checksum::sha1(&raw.sha1)
        .ok_or_else(|| ProviderError::invalid(SOURCE, "sha1 của server.jar sai định dạng"))?;
    Ok(ServerJar {
        url: raw.url,
        checksum,
        size: raw.size,
    })
}
