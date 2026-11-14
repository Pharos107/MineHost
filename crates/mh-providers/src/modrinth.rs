//! Modrinth API v2 (spec P3, mục 3.5, 3.7, 5.3, 5.4).

use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use mh_core::{LoaderKind, McVersion};
use mh_net::{Checksum, HostAllowlist, HttpClient, NetError, hosts};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;

use crate::ProviderError;

pub const API_BASE: &str = "https://api.modrinth.com/v2";
const SOURCE: &str = "Modrinth";
const SEARCH_TTL: Duration = Duration::from_secs(10 * 60);
const PROJECT_TTL: Duration = Duration::from_secs(60 * 60);
const DEFAULT_LIMIT: u32 = 20;
const MAX_LIMIT: u32 = 100;

#[derive(Debug, Clone)]
pub struct Modrinth {
    http: Arc<HttpClient>,
    base_url: String,
    hosts: HostAllowlist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectType {
    Mod,
    Plugin,
    Modpack,
}

impl ProjectType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Plugin => "plugin",
            Self::Modpack => "modpack",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SearchIndex {
    #[default]
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
}

impl SearchIndex {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Downloads => "downloads",
            Self::Follows => "follows",
            Self::Newest => "newest",
            Self::Updated => "updated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFilters {
    pub project_type: Option<ProjectType>,
    pub loader: Option<LoaderKind>,
    pub minecraft: Option<McVersion>,
    /// Lọc `server_side != unsupported` khi tìm nội dung cài cho server.
    pub server_only: bool,
    pub index: SearchIndex,
    pub offset: u32,
    pub limit: u32,
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            project_type: None,
            loader: None,
            minecraft: None,
            server_only: false,
            index: SearchIndex::Relevance,
            offset: 0,
            limit: DEFAULT_LIMIT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SideSupport {
    Required,
    Optional,
    Unsupported,
    Unknown,
}

impl SideSupport {
    pub const fn is_unsupported(self) -> bool {
        matches!(self, Self::Unsupported)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub project_type: String,
    pub title: String,
    pub description: String,
    #[serde(default, deserialize_with = "non_empty_opt")]
    pub icon_url: Option<String>,
    pub downloads: u64,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub versions: Vec<String>,
    pub client_side: SideSupport,
    pub server_side: SideSupport,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProjectLinks {
    #[serde(default, deserialize_with = "non_empty_opt")]
    pub source_url: Option<String>,
    #[serde(default, deserialize_with = "non_empty_opt")]
    pub issues_url: Option<String>,
    #[serde(default, deserialize_with = "non_empty_opt")]
    pub wiki_url: Option<String>,
    #[serde(default, deserialize_with = "non_empty_opt")]
    pub discord_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub project_type: String,
    pub title: String,
    pub description: String,
    pub body: String,
    #[serde(default, deserialize_with = "non_empty_opt")]
    pub icon_url: Option<String>,
    pub downloads: u64,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    pub client_side: SideSupport,
    pub server_side: SideSupport,
    #[serde(flatten)]
    pub links: ProjectLinks,
}

impl Project {
    pub fn page_url(&self) -> String {
        format!("https://modrinth.com/{}/{}", self.project_type, self.slug)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FileHashes {
    pub sha1: String,
    pub sha512: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    pub size: u64,
    pub hashes: FileHashes,
    #[serde(default)]
    pub primary: bool,
}

impl VersionFile {
    /// Checksum dùng khi tải về; ưu tiên SHA-512 theo spec P3, mục 5.3.
    pub fn checksum(&self) -> Option<Checksum> {
        Checksum::sha512(&self.hashes.sha512).or_else(|| Checksum::sha1(&self.hashes.sha1))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Dependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub dependency_type: DependencyType,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModrinthVersion {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub date_published: OffsetDateTime,
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

impl ModrinthVersion {
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }
}

#[derive(Serialize)]
struct HashLookupRequest<'a> {
    hashes: &'a [&'a str],
    algorithm: &'static str,
}

impl Modrinth {
    pub fn new(http: Arc<HttpClient>) -> Self {
        Self::with_base_url(http, API_BASE)
    }

    /// Dùng trong test để trỏ tới server giả.
    pub fn with_base_url(http: Arc<HttpClient>, base_url: impl Into<String>) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            hosts: hosts::MODRINTH,
        }
    }

    /// Dùng trong test khi file tải nằm trên host giả khác `hosts::MODRINTH`.
    pub fn with_hosts(mut self, hosts: HostAllowlist) -> Self {
        self.hosts = hosts;
        self
    }

    pub async fn search(
        &self,
        query: &str,
        filters: &SearchFilters,
    ) -> Result<SearchResult, ProviderError> {
        let mut url = endpoint(&self.base_url, &["search"])?;
        let facets = build_facets(filters);
        let limit = if filters.limit == 0 {
            DEFAULT_LIMIT
        } else {
            filters.limit.min(MAX_LIMIT)
        };
        {
            let mut qp = url.query_pairs_mut();
            if !query.is_empty() {
                qp.append_pair("query", query);
            }
            if !facets.is_empty() {
                let encoded = serde_json::to_string(&facets)
                    .map_err(|_| ProviderError::invalid(SOURCE, "không tạo được facets"))?;
                qp.append_pair("facets", &encoded);
            }
            qp.append_pair("index", filters.index.as_str());
            qp.append_pair("offset", &filters.offset.to_string());
            qp.append_pair("limit", &limit.to_string());
        }
        self.get_json(url.as_str(), SEARCH_TTL, query).await
    }

    pub async fn project(&self, id_or_slug: &str) -> Result<Project, ProviderError> {
        if id_or_slug.trim().is_empty() {
            return Err(not_found(id_or_slug));
        }
        let url = endpoint(&self.base_url, &["project", id_or_slug])?;
        self.get_json(url.as_str(), PROJECT_TTL, id_or_slug).await
    }

    /// Các phiên bản hợp `loader` và `mc`, mới nhất trước.
    pub async fn versions(
        &self,
        project: &str,
        loader: LoaderKind,
        mc: &McVersion,
    ) -> Result<Vec<ModrinthVersion>, ProviderError> {
        if project.trim().is_empty() {
            return Err(not_found(project));
        }
        let loaders = modrinth_loaders(loader);
        if loaders.is_empty() {
            return Ok(Vec::new());
        }
        let loaders_json = serde_json::to_string(loaders)
            .map_err(|_| ProviderError::invalid(SOURCE, "không tạo được tham số loaders"))?;
        let mc_json = serde_json::to_string(&[mc.as_str()])
            .map_err(|_| ProviderError::invalid(SOURCE, "không tạo được tham số game_versions"))?;
        let mut url = endpoint(&self.base_url, &["project", project, "version"])?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("loaders", &loaders_json);
            qp.append_pair("game_versions", &mc_json);
        }
        let all: Vec<ModrinthVersion> =
            self.get_json(url.as_str(), Duration::ZERO, project).await?;
        // Một bản có file lạ không được làm mất cả danh sách: bỏ riêng bản đó.
        let mut out: Vec<ModrinthVersion> = all
            .into_iter()
            .filter_map(|ver| keep_valid(&self.hosts, ver))
            .collect();
        out.sort_by_key(|v| Reverse(v.date_published));
        Ok(out)
    }

    pub async fn version(&self, id: &str) -> Result<ModrinthVersion, ProviderError> {
        if id.trim().is_empty() {
            return Err(not_found(id));
        }
        let url = endpoint(&self.base_url, &["version", id])?;
        let mut ver: ModrinthVersion = self.get_json(url.as_str(), Duration::ZERO, id).await?;
        validate_version(&self.hosts, &mut ver)?;
        Ok(ver)
    }

    /// Tra phiên bản theo danh sách SHA-1. Hash không có trên Modrinth sẽ vắng mặt trong map.
    pub async fn versions_by_hash<S: AsRef<str>>(
        &self,
        hashes: &[S],
    ) -> Result<HashMap<String, ModrinthVersion>, ProviderError> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        let refs: Vec<&str> = hashes.iter().map(AsRef::as_ref).collect();
        let url = endpoint(&self.base_url, &["version_files"])?;
        let req = HashLookupRequest {
            hashes: &refs,
            algorithm: "sha1",
        };
        let all: HashMap<String, ModrinthVersion> = self
            .http
            .post_json(url.as_str(), &req)
            .await
            .map_err(|e| map_net_err(e, ""))?;
        Ok(all
            .into_iter()
            .filter_map(|(hash, ver)| keep_valid(&self.hosts, ver).map(|ver| (hash, ver)))
            .collect())
    }

    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        ttl: Duration,
        id: &str,
    ) -> Result<T, ProviderError> {
        self.http
            .get_json(url, ttl)
            .await
            .map(|f| f.value)
            .map_err(|e| map_net_err(e, id))
    }
}

/// Ánh xạ `LoaderKind` sang tên loader trên Modrinth.
///
/// Paper chạy được plugin của cả Paper, Purpur, Spigot và Bukkit (Purpur cũng vậy).
pub fn modrinth_loaders(loader: LoaderKind) -> &'static [&'static str] {
    match loader {
        LoaderKind::Paper | LoaderKind::Purpur => &["paper", "purpur", "spigot", "bukkit"],
        LoaderKind::Fabric => &["fabric"],
        LoaderKind::Quilt => &["quilt"],
        LoaderKind::Forge => &["forge"],
        LoaderKind::NeoForge => &["neoforge"],
        LoaderKind::Vanilla => &[],
        _ => &[],
    }
}

fn build_facets(filters: &SearchFilters) -> Vec<Vec<String>> {
    let mut facets = Vec::new();
    if let Some(kind) = filters.project_type {
        facets.push(vec![format!("project_type:{}", kind.as_str())]);
    }
    if let Some(loader) = filters.loader {
        let group: Vec<String> = modrinth_loaders(loader)
            .iter()
            .map(|l| format!("categories:{l}"))
            .collect();
        if !group.is_empty() {
            facets.push(group);
        }
    }
    if let Some(mc) = &filters.minecraft {
        facets.push(vec![format!("versions:{mc}")]);
    }
    if filters.server_only {
        facets.push(vec!["server_side!=unsupported".to_owned()]);
    }
    facets
}

/// Giữ bản hợp lệ; bản có file ngoài danh sách host hoặc hash sai thì ghi log và bỏ.
fn keep_valid(hosts: &HostAllowlist, mut ver: ModrinthVersion) -> Option<ModrinthVersion> {
    match validate_version(hosts, &mut ver) {
        Ok(()) => Some(ver),
        Err(e) => {
            tracing::warn!(version = ver.id, error = %e, "bỏ bản Modrinth không hợp lệ");
            None
        }
    }
}

fn validate_version(hosts: &HostAllowlist, ver: &mut ModrinthVersion) -> Result<(), ProviderError> {
    for file in &mut ver.files {
        hosts.check_str(&file.url).map_err(|e| {
            tracing::warn!(url = file.url, error = %e, "Modrinth trỏ file ra ngoài danh sách host");
            ProviderError::Net(e)
        })?;
        if file.filename.is_empty() {
            return Err(ProviderError::invalid(
                SOURCE,
                "file Modrinth thiếu filename",
            ));
        }
        let sha1 = Checksum::sha1(&file.hashes.sha1).ok_or_else(|| {
            ProviderError::invalid(SOURCE, "sha1 của file Modrinth sai định dạng")
        })?;
        let sha512 = Checksum::sha512(&file.hashes.sha512).ok_or_else(|| {
            ProviderError::invalid(SOURCE, "sha512 của file Modrinth sai định dạng")
        })?;
        file.hashes.sha1 = sha1.hex().to_owned();
        file.hashes.sha512 = sha512.hex().to_owned();
    }
    Ok(())
}

fn endpoint(base: &str, segments: &[&str]) -> Result<Url, ProviderError> {
    let mut url =
        Url::parse(base).map_err(|_| ProviderError::invalid(SOURCE, "base URL không hợp lệ"))?;
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|()| ProviderError::invalid(SOURCE, "base URL không hỗ trợ path"))?;
        path.pop_if_empty();
        for seg in segments {
            path.push(seg);
        }
    }
    Ok(url)
}

fn map_net_err(err: NetError, id: &str) -> ProviderError {
    match err {
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

fn non_empty_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let opt = Option::<String>::deserialize(d)?;
    Ok(opt.filter(|s| !s.trim().is_empty()))
}
