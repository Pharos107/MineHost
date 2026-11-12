//! Purpur qua API v2 (spec P2, mục 5.4). Purpur là bản sửa của Paper nên chạy giống Paper.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use mh_core::McVersion;
use mh_net::{Checksum, HttpClient};
use serde::Deserialize;

use crate::{ProviderError, Release, ServerJar};

pub const API_BASE: &str = "https://api.purpurmc.org/v2/purpur";
const SOURCE: &str = "Purpur";
const PROJECT_TTL: Duration = Duration::from_secs(60 * 60);
const LATEST_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone)]
pub struct Purpur {
    http: Arc<HttpClient>,
    base_url: String,
}

/// Build Purpur sẽ cài cho một phiên bản Minecraft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurpurBuild {
    pub version: McVersion,
    pub build: u32,
    pub jar: ServerJar,
}

#[derive(Deserialize)]
struct RawProject {
    versions: Vec<String>,
}

#[derive(Deserialize)]
struct RawBuild {
    /// API trả số build dạng chuỗi, ví dụ `"2416"`.
    build: String,
    result: String,
    md5: String,
}

impl Purpur {
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

    /// Các bản release của Mojang mà Purpur có, giữ thứ tự của `releases` (mới nhất trước).
    pub async fn versions(&self, releases: &[Release]) -> Result<Vec<Release>, ProviderError> {
        let project: RawProject = self.http.get_json(&self.base_url, PROJECT_TTL).await?.value;
        let known: HashSet<String> = project.versions.into_iter().collect();
        Ok(releases
            .iter()
            .filter(|r| known.contains(r.id.as_str()))
            .cloned()
            .collect())
    }

    /// Build mới nhất của `version`. Build lỗi (`result` khác `SUCCESS`) thì không cài.
    pub async fn latest_build(&self, version: &McVersion) -> Result<PurpurBuild, ProviderError> {
        let url = format!("{}/{version}/latest", self.base_url);
        let raw: RawBuild = self.http.get_json(&url, LATEST_TTL).await?.value;
        if raw.result != "SUCCESS" {
            return Err(ProviderError::VersionNotFound {
                source_name: SOURCE,
                version: version.to_string(),
            });
        }
        let build: u32 = raw
            .build
            .parse()
            .map_err(|_| ProviderError::invalid(SOURCE, "số build không phải số"))?;
        let checksum = Checksum::md5(&raw.md5)
            .ok_or_else(|| ProviderError::invalid(SOURCE, "md5 sai định dạng"))?;
        Ok(PurpurBuild {
            version: version.clone(),
            build,
            jar: ServerJar {
                // Tải đúng build vừa đọc, không dùng `/latest/download` kẻo build mới ra giữa chừng.
                url: format!("{}/{version}/{build}/download", self.base_url),
                checksum,
                // API không ghi kích thước; tiến độ lấy từ `Content-Length` khi tải.
                size: 0,
            },
        })
    }
}
