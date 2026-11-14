//! CurseForge Core API v1 (spec P3, mục 3.6, 3.7, 5.3, 5.4, 11).

mod types;

use std::cmp::Reverse;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use mh_core::{LoaderKind, McVersion};
use mh_net::{Checksum, HostAllowlist, HttpClient, NetError, hosts};
use serde::{Deserialize, Serialize};

use crate::ProviderError;
pub use types::{
    ClassId, FileDependency, ModFile, ModInfo, ModLinks, ModLogo, Pagination, RelationType,
    ReleaseType, SearchFilters, SearchResult, SortField,
};
use types::{RawFile, RawFileHash};

pub const API_BASE: &str = "https://api.curseforge.com/v1";
pub const GAME_ID_MINECRAFT: u32 = 432;
pub const CLASS_MOD: u32 = 6;
pub const CLASS_MODPACK: u32 = 4471;

const SOURCE: &str = "CurseForge";
const API_KEY_HEADER: &str = "x-api-key";
const SEARCH_TTL: Duration = Duration::from_secs(10 * 60);
const MOD_TTL: Duration = Duration::from_secs(60 * 60);
const DEFAULT_PAGE_SIZE: u32 = 20;
const MAX_PAGE_SIZE: u32 = 50;
const HASH_ALGO_SHA1: u32 = 1;

#[derive(Clone)]
pub struct CurseForge {
    http: Arc<HttpClient>,
    api_key: String,
    base_url: String,
    hosts: HostAllowlist,
}

impl fmt::Debug for CurseForge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Không bao giờ để lộ key API qua log hay Debug.
        let key_state = if self.api_key.trim().is_empty() {
            "<empty>"
        } else {
            "<redacted>"
        };
        f.debug_struct("CurseForge")
            .field("base_url", &self.base_url)
            .field("api_key", &key_state)
            .field("hosts", &self.hosts)
            .finish()
    }
}

#[derive(Deserialize)]
struct DataEnvelope<T> {
    data: T,
}

#[derive(Deserialize)]
struct SearchResponse {
    data: Vec<ModInfo>,
    pagination: Pagination,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FilesByIdRequest<'a> {
    file_ids: &'a [u64],
}

impl CurseForge {
    pub fn new(http: Arc<HttpClient>, api_key: impl Into<String>) -> Self {
        Self::with_base_url(http, api_key, API_BASE)
    }

    /// Dùng trong test để trỏ tới server giả.
    pub fn with_base_url(
        http: Arc<HttpClient>,
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            http,
            api_key: api_key.into(),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            hosts: hosts::CURSEFORGE,
        }
    }

    /// Dùng trong test khi file tải nằm trên host giả khác `hosts::CURSEFORGE`.
    pub fn with_hosts(mut self, hosts: HostAllowlist) -> Self {
        self.hosts = hosts;
        self
    }

    pub async fn search(
        &self,
        query: &str,
        filters: &SearchFilters,
    ) -> Result<SearchResult, ProviderError> {
        let key = self.require_key()?;
        let loader_type = match filters.loader {
            Some(l) => match mod_loader_type(l) {
                Some(id) => Some(id),
                None => {
                    return Ok(SearchResult {
                        hits: Vec::new(),
                        pagination: Pagination::default(),
                    });
                }
            },
            None => None,
        };
        let page_size = if filters.page_size == 0 {
            DEFAULT_PAGE_SIZE
        } else {
            filters.page_size.min(MAX_PAGE_SIZE)
        };
        let mut url = crate::endpoint(SOURCE, &self.base_url, &["mods", "search"])?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("gameId", &GAME_ID_MINECRAFT.to_string());
            if let Some(class_id) = filters.class_id {
                qp.append_pair("classId", &class_id.as_u32().to_string());
            }
            if let Some(lt) = loader_type {
                qp.append_pair("modLoaderType", &lt.to_string());
            }
            if let Some(mc) = &filters.minecraft {
                qp.append_pair("gameVersion", mc.as_str());
            }
            if !query.trim().is_empty() {
                qp.append_pair("searchFilter", query);
            }
            qp.append_pair("sortField", &(filters.sort_field as u32).to_string());
            qp.append_pair("index", &filters.index.to_string());
            qp.append_pair("pageSize", &page_size.to_string());
        }
        let resp: SearchResponse = self.get_json(url.as_str(), SEARCH_TTL, key, query).await?;
        Ok(SearchResult {
            hits: resp.data,
            pagination: resp.pagination,
        })
    }

    pub async fn mod_info(&self, id: u64) -> Result<ModInfo, ProviderError> {
        let key = self.require_key()?;
        if id == 0 {
            return Err(not_found("0"));
        }
        let id_str = id.to_string();
        let url = crate::endpoint(SOURCE, &self.base_url, &["mods", &id_str])?;
        let env: DataEnvelope<ModInfo> = self.get_json(url.as_str(), MOD_TTL, key, &id_str).await?;
        Ok(env.data)
    }

    /// Danh sách file hợp `loader` và `mc`, mới nhất trước.
    pub async fn files(
        &self,
        mod_id: u64,
        loader: LoaderKind,
        mc: &McVersion,
    ) -> Result<Vec<ModFile>, ProviderError> {
        let key = self.require_key()?;
        if mod_id == 0 {
            return Err(not_found("0"));
        }
        let Some(loader_type) = mod_loader_type(loader) else {
            return Ok(Vec::new());
        };
        let mod_info = self.mod_info(mod_id).await?;
        let id_str = mod_id.to_string();
        let mut url = crate::endpoint(SOURCE, &self.base_url, &["mods", &id_str, "files"])?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("gameVersion", mc.as_str());
            qp.append_pair("modLoaderType", &loader_type.to_string());
        }
        let env: DataEnvelope<Vec<RawFile>> = self
            .get_json(url.as_str(), Duration::ZERO, key, &id_str)
            .await?;
        let mut out: Vec<ModFile> = env
            .data
            .into_iter()
            .filter_map(|raw| keep_valid(&self.hosts, raw, Some(&mod_info)))
            .collect();
        out.sort_by_key(|f| Reverse(f.file_date));
        Ok(out)
    }

    pub async fn files_by_id(&self, ids: &[u64]) -> Result<Vec<ModFile>, ProviderError> {
        let key = self.require_key()?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let url = crate::endpoint(SOURCE, &self.base_url, &["mods", "files"])?;
        let req = FilesByIdRequest { file_ids: ids };
        let env: DataEnvelope<Vec<RawFile>> = self
            .http
            .post_json_with_headers(url.as_str(), &req, &[(API_KEY_HEADER, key)])
            .await
            .map_err(|e| map_net_err(e, ""))?;
        Ok(env
            .data
            .into_iter()
            .filter_map(|raw| keep_valid(&self.hosts, raw, None))
            .collect())
    }

    fn require_key(&self) -> Result<&str, ProviderError> {
        if self.api_key.trim().is_empty() {
            return Err(ProviderError::CurseforgeUnavailable);
        }
        Ok(&self.api_key)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        ttl: Duration,
        key: &str,
        id: &str,
    ) -> Result<T, ProviderError> {
        self.http
            .get_json_with_headers(url, ttl, &[(API_KEY_HEADER, key)])
            .await
            .map(|f| f.value)
            .map_err(|e| map_net_err(e, id))
    }
}

/// Ánh xạ `LoaderKind` sang `ModLoaderType` của CurseForge.
///
/// Plugin Bukkit/Paper/Purpur không lấy từ CurseForge trong P3.
pub const fn mod_loader_type(loader: LoaderKind) -> Option<u32> {
    match loader {
        LoaderKind::Forge => Some(1),
        LoaderKind::Fabric => Some(4),
        LoaderKind::Quilt => Some(5),
        LoaderKind::NeoForge => Some(6),
        LoaderKind::Paper | LoaderKind::Purpur | LoaderKind::Vanilla => None,
        _ => None,
    }
}

fn keep_valid(hosts: &HostAllowlist, raw: RawFile, mod_info: Option<&ModInfo>) -> Option<ModFile> {
    let file_id = raw.file.id;
    match validate_file(hosts, raw, mod_info) {
        Ok(file) => Some(file),
        Err(e) => {
            tracing::warn!(file_id, error = %e, "bỏ file CurseForge không hợp lệ");
            None
        }
    }
}

fn validate_file(
    hosts: &HostAllowlist,
    raw: RawFile,
    mod_info: Option<&ModInfo>,
) -> Result<ModFile, ProviderError> {
    let mut file = raw.file;
    if file.file_name.trim().is_empty() {
        return Err(ProviderError::invalid(
            SOURCE,
            "file CurseForge thiếu fileName",
        ));
    }
    if let Some(url) = file.download_url.as_deref() {
        hosts.check_str(url).map_err(|e| {
            tracing::warn!(url, error = %e, "CurseForge trỏ file ra ngoài danh sách host");
            ProviderError::Net(e)
        })?;
    }
    file.sha1 = extract_sha1(&raw.hashes)?;
    file.manual_download =
        file.download_url.is_none() || mod_info.is_some_and(|m| !m.allows_distribution());
    file.download_page_url = mod_info.map(|m| m.download_page_url(file.id));
    Ok(file)
}

fn extract_sha1(hashes: &[RawFileHash]) -> Result<Option<String>, ProviderError> {
    let Some(h) = hashes.iter().find(|h| h.algo == HASH_ALGO_SHA1) else {
        return Ok(None);
    };
    let sum = Checksum::sha1(&h.value)
        .ok_or_else(|| ProviderError::invalid(SOURCE, "sha1 của file CurseForge sai định dạng"))?;
    Ok(Some(sum.hex().to_owned()))
}

fn map_net_err(err: NetError, id: &str) -> ProviderError {
    match err {
        NetError::Status {
            status: 401 | 403, ..
        } => {
            tracing::warn!("CurseForge từ chối API key");
            ProviderError::CurseforgeUnavailable
        }
        NetError::Status { status: 404, .. } => not_found(id),
        other => ProviderError::Net(other),
    }
}

fn not_found(id: &str) -> ProviderError {
    ProviderError::ContentNotFound {
        source_name: SOURCE,
        id: id.to_owned(),
    }
}
