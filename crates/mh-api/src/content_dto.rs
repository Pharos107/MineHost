//! Kiểu dữ liệu (DTO) và tham số truy vấn cho route mod/plugin.

use std::path::Path;

use mh_app::content::{
    CatalogSource, ContentFile, ContentInstallPlan, ContentList, ContentProject, ContentSearchHit,
    ContentSource, ContentVersion, PendingContent, PlannedContent, SideSupport,
};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;

use crate::dto::ContentKindDto;

pub(crate) const SOURCE_MODRINTH: &str = CatalogSource::Modrinth.as_str();

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct SearchQuery {
    /// Nguồn tìm kiếm; mặc định `modrinth`.
    #[serde(default)]
    pub source: Option<String>,
    /// `mod`, `plugin` hoặc `modpack`.
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
    /// Lọc theo loader, Minecraft của server và `server_side != unsupported`.
    #[serde(default)]
    pub server_id: Option<String>,
    /// Trang (từ 1), mỗi trang 20 kết quả.
    #[serde(default)]
    pub page: Option<u32>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct VersionsQuery {
    pub server_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SideSupportDto {
    Required,
    Optional,
    Unsupported,
    Unknown,
}

impl From<SideSupport> for SideSupportDto {
    fn from(s: SideSupport) -> Self {
        match s {
            SideSupport::Required => Self::Required,
            SideSupport::Optional => Self::Optional,
            SideSupport::Unsupported => Self::Unsupported,
            SideSupport::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SearchHitDto {
    pub source: String,
    pub project_id: String,
    pub slug: String,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub categories: Vec<String>,
    pub game_versions: Vec<String>,
    /// Chỉ Modrinth trả thông tin này; CurseForge để `null`.
    pub server_side: Option<SideSupportDto>,
}

impl From<ContentSearchHit> for SearchHitDto {
    fn from(hit: ContentSearchHit) -> Self {
        Self {
            source: hit.source.as_str().to_owned(),
            project_id: hit.project_id,
            slug: hit.slug,
            kind: hit.project_type,
            title: hit.title,
            description: hit.description,
            icon_url: hit.icon_url,
            downloads: hit.downloads,
            categories: hit.categories,
            game_versions: hit.game_versions,
            server_side: hit.server_side.map(Into::into),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResultDto {
    pub hits: Vec<SearchHitDto>,
    pub page: u32,
    pub page_size: u32,
    pub total: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectLinksDto {
    pub page: String,
    pub source: Option<String>,
    pub issues: Option<String>,
    pub wiki: Option<String>,
    pub discord: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectDto {
    pub source: String,
    pub project_id: String,
    pub slug: String,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub body: Option<String>,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub client_side: Option<SideSupportDto>,
    pub server_side: Option<SideSupportDto>,
    pub links: ProjectLinksDto,
}

impl From<ContentProject> for ProjectDto {
    fn from(p: ContentProject) -> Self {
        Self {
            source: p.source.as_str().to_owned(),
            project_id: p.id,
            slug: p.slug,
            kind: p.project_type,
            title: p.title,
            description: p.description,
            body: p.body,
            icon_url: p.icon_url,
            downloads: p.downloads,
            loaders: p.loaders,
            game_versions: p.game_versions,
            client_side: p.client_side.map(Into::into),
            server_side: p.server_side.map(Into::into),
            links: ProjectLinksDto {
                page: p.page_url,
                source: p.links.source_url,
                issues: p.links.issues_url,
                wiki: p.links.wiki_url,
                discord: p.links.discord_url,
            },
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ContentVersionDto {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: Option<String>,
    pub version_type: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    /// RFC 3339.
    pub date_published: String,
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
    pub manual_download: bool,
    pub download_page_url: Option<String>,
}

impl From<ContentVersion> for ContentVersionDto {
    fn from(v: ContentVersion) -> Self {
        let primary = v.primary_file().cloned();
        Self {
            id: v.id,
            project_id: v.project_id,
            name: v.name,
            version_number: v.version_number,
            version_type: v.version_type,
            game_versions: v.game_versions,
            loaders: v.loaders,
            date_published: v.date_published.format(&Rfc3339).unwrap_or_default(),
            file_name: primary.as_ref().map(|f| f.filename.clone()),
            file_size: primary.as_ref().map(|f| f.size),
            manual_download: primary.as_ref().is_some_and(|f| f.manual_download),
            download_page_url: primary.and_then(|f| f.download_page_url),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContentSourceDto {
    Modrinth,
    Curseforge,
    Modpack,
    Upload,
}

impl From<ContentSource> for ContentSourceDto {
    fn from(s: ContentSource) -> Self {
        match s {
            ContentSource::Modrinth => Self::Modrinth,
            ContentSource::Curseforge => Self::Curseforge,
            ContentSource::Modpack => Self::Modpack,
            ContentSource::Upload => Self::Upload,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InstalledContentDto {
    pub file_name: String,
    pub name: String,
    pub version: Option<String>,
    pub enabled: bool,
    pub size: u64,
    pub source: Option<ContentSourceDto>,
    /// `true` khi người dùng chép trực tiếp vào thư mục mà chưa có bản ghi nguồn.
    pub manual: bool,
    pub client_only: bool,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub from_modpack: bool,
    pub icon_url: Option<String>,
}

impl From<ContentFile> for InstalledContentDto {
    fn from(file: ContentFile) -> Self {
        let meta = file.meta.ok();
        let name = meta
            .as_ref()
            .map(|m| m.name.clone())
            .unwrap_or_else(|| file.file_name.clone());
        let version = meta.as_ref().and_then(|m| m.version.clone());
        let client_only = file.record.as_ref().is_some_and(|r| r.client_only)
            || meta.as_ref().is_some_and(|m| m.side.is_client_only());
        let rec = file.record;
        Self {
            file_name: file.file_name,
            name,
            version,
            enabled: file.enabled,
            size: file.size,
            source: rec.as_ref().map(|r| r.source.into()),
            manual: file.manual,
            client_only,
            project_id: rec
                .as_ref()
                .and_then(|r| r.project_id.as_ref())
                .map(ToString::to_string),
            version_id: rec.as_ref().and_then(|r| r.version_id.clone()),
            from_modpack: rec.is_some_and(|r| r.from_modpack),
            icon_url: None,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PendingContentDto {
    pub path: String,
    pub file_name: String,
    pub project_id: u64,
    pub file_id: u64,
    pub sha1: String,
    pub page_url: String,
}

impl From<PendingContent> for PendingContentDto {
    fn from(p: PendingContent) -> Self {
        let file_name = Path::new(&p.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&p.path)
            .to_owned();
        Self {
            path: p.path,
            file_name,
            project_id: p.project_id,
            file_id: p.file_id,
            sha1: p.sha1,
            page_url: p.page_url,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ServerContentDto {
    pub kind: ContentKindDto,
    pub files: Vec<InstalledContentDto>,
    pub pending: Vec<PendingContentDto>,
}

impl From<ContentList> for ServerContentDto {
    fn from(list: ContentList) -> Self {
        Self {
            kind: list.kind.into(),
            files: list.files.into_iter().map(Into::into).collect(),
            pending: list.pending.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ContentInstallBody {
    pub source: String,
    pub project_id: String,
    pub version_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PlannedContentDto {
    pub project_id: String,
    pub project_name: String,
    pub version_id: String,
    pub version_number: String,
    pub file_name: String,
    pub file_size: u64,
    pub client_only: bool,
    pub replaces_existing: bool,
    pub is_dependency: bool,
    pub manual_download: bool,
    pub download_page_url: Option<String>,
}

impl From<PlannedContent> for PlannedContentDto {
    fn from(p: PlannedContent) -> Self {
        Self {
            project_id: p.project_id,
            project_name: p.project_name,
            version_id: p.version_id,
            version_number: p.version_number,
            file_name: p.file.filename,
            file_size: p.file.size,
            client_only: p.client_only,
            replaces_existing: p.replaces_existing,
            is_dependency: p.is_dependency,
            manual_download: p.file.manual_download,
            download_page_url: p.file.download_page_url,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ContentPlanDto {
    pub kind: ContentKindDto,
    pub items: Vec<PlannedContentDto>,
}

impl From<ContentInstallPlan> for ContentPlanDto {
    fn from(plan: ContentInstallPlan) -> Self {
        Self {
            kind: plan.kind.into(),
            items: plan.items.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ToggleContentBody {
    pub enabled: bool,
}
