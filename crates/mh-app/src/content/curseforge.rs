//! Nguồn danh mục CurseForge: map API CurseForge về kiểu nội dung chung.

use std::path::Path;
use std::sync::Arc;

use mh_core::{ErrorCode, LoaderKind, McVersion};
use mh_net::{HostAllowlist, HttpClient, hosts};
pub use mh_providers::curseforge::CurseForge;
use mh_providers::curseforge::{
    self as cf, ClassId, ModFile, ModInfo, RelationType, ReleaseType, SortField,
};
use mh_providers::modrinth::{
    DependencyType, ProjectLinks, ProjectType, SearchFilters, SearchIndex,
};

use super::catalog::{
    CatalogSource, ContentCatalog, ContentDependency, ContentFileInfo, ContentProject,
    ContentSearchHit, ContentSearchResult, ContentVersion, download_with_hosts,
};
use crate::AppError;
use crate::sources::{BoxFut, CancellationToken, ProgressFn};

/// Nguồn CurseForge thật: dùng `HttpClient` chung để tận dụng cache và kiểm soát host.
#[derive(Debug, Clone)]
pub struct CurseforgeCatalog {
    http: Arc<HttpClient>,
    cf: CurseForge,
    hosts: HostAllowlist,
}

impl CurseforgeCatalog {
    pub fn new(http: Arc<HttpClient>, api_key: impl Into<String>) -> Self {
        let cf = CurseForge::new(Arc::clone(&http), api_key);
        Self {
            http,
            cf,
            hosts: hosts::CURSEFORGE,
        }
    }

    pub fn with_curseforge(mut self, cf: CurseForge, hosts: HostAllowlist) -> Self {
        self.cf = cf;
        self.hosts = hosts;
        self
    }
}

impl ContentCatalog for CurseforgeCatalog {
    fn search<'a>(
        &'a self,
        query: &'a str,
        filters: &'a SearchFilters,
    ) -> BoxFut<'a, Result<ContentSearchResult, AppError>> {
        Box::pin(async move {
            // Spec P3 11.6: plugin Bukkit không lấy từ CurseForge.
            if matches!(filters.project_type, Some(ProjectType::Plugin))
                || filters
                    .loader
                    .is_some_and(|l| cf::mod_loader_type(l).is_none())
            {
                return Ok(ContentSearchResult {
                    hits: Vec::new(),
                    offset: filters.offset,
                    limit: filters.limit,
                    total_hits: 0,
                });
            }
            let page_size = if filters.limit == 0 {
                20
            } else {
                filters.limit.min(50)
            };
            let cf_filters = cf::SearchFilters {
                class_id: Some(match filters.project_type {
                    Some(ProjectType::Modpack) => ClassId::Modpack,
                    _ => ClassId::Mod,
                }),
                loader: filters.loader,
                minecraft: filters.minecraft.clone(),
                sort_field: match filters.index {
                    SearchIndex::Relevance => SortField::Featured,
                    SearchIndex::Downloads => SortField::TotalDownloads,
                    SearchIndex::Follows => SortField::Popularity,
                    SearchIndex::Newest => SortField::ReleasedDate,
                    SearchIndex::Updated => SortField::LastUpdated,
                },
                index: filters.offset,
                page_size,
            };
            let res = self.cf.search(query, &cf_filters).await?;
            Ok(ContentSearchResult {
                hits: res.hits.into_iter().map(cf_hit).collect(),
                offset: res.pagination.index,
                limit: res.pagination.page_size,
                total_hits: res.pagination.total_count,
            })
        })
    }

    fn project<'a>(&'a self, id_or_slug: &'a str) -> BoxFut<'a, Result<ContentProject, AppError>> {
        Box::pin(async move {
            let id = parse_cf_id(id_or_slug, "project_id")?;
            Ok(cf_project(self.cf.mod_info(id).await?))
        })
    }

    fn versions<'a>(
        &'a self,
        project: &'a str,
        loader: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<ContentVersion>, AppError>> {
        Box::pin(async move {
            let id = parse_cf_id(project, "project_id")?;
            let files = self.cf.files(id, loader, minecraft).await?;
            Ok(files
                .into_iter()
                .filter(|f| !f.is_server_pack)
                .map(cf_version)
                .collect())
        })
    }

    fn version<'a>(&'a self, id: &'a str) -> BoxFut<'a, Result<ContentVersion, AppError>> {
        Box::pin(async move {
            let fid = parse_cf_id(id, "version_id")?;
            let mut file = self
                .cf
                .files_by_id(&[fid])
                .await?
                .into_iter()
                .next()
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, id))?;
            // `files_by_id` không kèm `allowModDistribution` và slug; lấy từ `mod_info` (được cache).
            let info = self.cf.mod_info(file.mod_id).await?;
            file.apply_mod(&info);
            Ok(cf_version(file))
        })
    }

    fn download<'a>(
        &'a self,
        file: &'a ContentFileInfo,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        download_with_hosts(&self.http, &self.hosts, file, dest, progress, cancel)
    }
}

pub(crate) fn parse_cf_id(raw: &str, field: &'static str) -> Result<u64, AppError> {
    raw.parse::<u64>().map_err(|_| {
        AppError::new(
            ErrorCode::InvalidInput,
            format!("{field} phải là số nguyên dương"),
        )
        .with_params(serde_json::json!({ "field": field }))
    })
}

fn cf_kind(m: &ModInfo) -> &'static str {
    if m.class_segment() == "modpacks" {
        "modpack"
    } else {
        "mod"
    }
}

fn cf_hit(m: ModInfo) -> ContentSearchHit {
    let kind = cf_kind(&m).to_owned();
    ContentSearchHit {
        source: CatalogSource::Curseforge,
        project_id: m.id.to_string(),
        slug: m.slug,
        project_type: kind,
        title: m.name,
        description: m.summary,
        icon_url: m.logo.and_then(|l| l.thumbnail_url.or(l.url)),
        downloads: m.download_count,
        categories: Vec::new(),
        game_versions: Vec::new(),
        server_side: None,
    }
}

fn cf_project(m: ModInfo) -> ContentProject {
    let kind = cf_kind(&m).to_owned();
    let page_url = m.links.website_url.clone().unwrap_or_else(|| {
        format!(
            "https://www.curseforge.com/minecraft/{}/{}",
            m.class_segment(),
            m.slug
        )
    });
    ContentProject {
        source: CatalogSource::Curseforge,
        id: m.id.to_string(),
        slug: m.slug,
        project_type: kind,
        title: m.name,
        description: m.summary,
        body: None,
        icon_url: m.logo.and_then(|l| l.thumbnail_url.or(l.url)),
        downloads: m.download_count,
        loaders: Vec::new(),
        game_versions: Vec::new(),
        client_side: None,
        server_side: None,
        page_url,
        links: ProjectLinks {
            source_url: m.links.source_url,
            issues_url: m.links.issues_url,
            wiki_url: m.links.wiki_url,
            discord_url: None,
        },
    }
}

fn cf_version(file: ModFile) -> ContentVersion {
    let (game_versions, loaders) = split_cf_game_versions(&file.game_versions);
    let version_type = match file.release_type {
        ReleaseType::Release => "release",
        ReleaseType::Beta => "beta",
        ReleaseType::Alpha => "alpha",
        ReleaseType::Unknown(_) => "unknown",
    }
    .to_owned();
    let dependencies = file
        .dependencies
        .iter()
        .filter_map(|d| {
            let dependency_type = match d.relation_type {
                RelationType::RequiredDependency => DependencyType::Required,
                RelationType::Incompatible => DependencyType::Incompatible,
                RelationType::OptionalDependency => DependencyType::Optional,
                RelationType::EmbeddedLibrary => DependencyType::Embedded,
                RelationType::Tool | RelationType::Include | RelationType::Unknown(_) => {
                    return None;
                }
            };
            Some(ContentDependency {
                project_id: Some(d.mod_id.to_string()),
                version_id: None,
                dependency_type,
            })
        })
        .collect();
    let url = file.api_download_url().map(str::to_owned);
    let info = ContentFileInfo {
        url,
        filename: file.file_name,
        size: file.file_length,
        sha1: file.sha1,
        sha512: None,
        manual_download: file.manual_download,
        download_page_url: file.download_page_url,
    };
    ContentVersion {
        id: file.id.to_string(),
        project_id: file.mod_id.to_string(),
        name: file.display_name,
        version_number: None,
        version_type,
        game_versions,
        loaders,
        date_published: file.file_date,
        dependencies,
        files: vec![info],
    }
}

fn split_cf_game_versions(raw: &[String]) -> (Vec<String>, Vec<String>) {
    let mut versions = Vec::new();
    let mut loaders = Vec::new();
    for tag in raw {
        let lower = tag.to_ascii_lowercase();
        match lower.as_str() {
            "fabric" | "forge" | "neoforge" | "quilt" => loaders.push(lower),
            _ => versions.push(tag.clone()),
        }
    }
    (versions, loaders)
}
