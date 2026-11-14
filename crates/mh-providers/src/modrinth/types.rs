//! Kiểu dữ liệu request/response của Modrinth API v2.

use mh_core::{LoaderKind, McVersion};
use mh_net::Checksum;
use serde::Deserialize;
use time::OffsetDateTime;

pub(super) const DEFAULT_LIMIT: u32 = 20;

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

fn non_empty_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let opt = Option::<String>::deserialize(d)?;
    Ok(opt.filter(|s| !s.trim().is_empty()))
}
