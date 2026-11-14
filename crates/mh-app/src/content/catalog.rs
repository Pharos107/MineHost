//! Nguồn danh mục mod/plugin dùng chung cho Modrinth và CurseForge.

use std::path::Path;
use std::sync::Arc;

use mh_core::{ErrorCode, LoaderKind, McVersion};
use mh_net::{Checksum, Download, HostAllowlist, HttpClient, hosts};
pub use mh_providers::modrinth::{
    DependencyType, Modrinth, ProjectLinks, ProjectType, SearchFilters, SearchIndex, SideSupport,
    modrinth_loaders,
};
use mh_providers::modrinth::{ModrinthVersion, Project, SearchHit, VersionFile};
use mh_store::{ContentSource, ProjectId};
use time::OffsetDateTime;

use super::curseforge::parse_cf_id;
use crate::AppError;
use crate::sources::{BoxFut, CancellationToken, ProgressFn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatalogSource {
    Modrinth,
    Curseforge,
}

impl CatalogSource {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "modrinth" => Some(Self::Modrinth),
            "curseforge" => Some(Self::Curseforge),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::Curseforge => "curseforge",
        }
    }

    pub const fn to_content_source(self) -> ContentSource {
        match self {
            Self::Modrinth => ContentSource::Modrinth,
            Self::Curseforge => ContentSource::Curseforge,
        }
    }

    pub fn project_id(self, raw: &str) -> Result<ProjectId, AppError> {
        match self {
            Self::Modrinth => Ok(ProjectId::Str(raw.to_owned())),
            Self::Curseforge => parse_cf_id(raw, "project_id").map(ProjectId::Int),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentSearchHit {
    pub source: CatalogSource,
    pub project_id: String,
    pub slug: String,
    pub project_type: String,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub categories: Vec<String>,
    pub game_versions: Vec<String>,
    pub server_side: Option<SideSupport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentSearchResult {
    pub hits: Vec<ContentSearchHit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentProject {
    pub source: CatalogSource,
    pub id: String,
    pub slug: String,
    pub project_type: String,
    pub title: String,
    pub description: String,
    pub body: Option<String>,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub client_side: Option<SideSupport>,
    pub server_side: Option<SideSupport>,
    pub page_url: String,
    pub links: ProjectLinks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub dependency_type: DependencyType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentFileInfo {
    pub url: Option<String>,
    pub filename: String,
    pub size: u64,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    pub manual_download: bool,
    pub download_page_url: Option<String>,
}

impl ContentFileInfo {
    pub fn checksum(&self) -> Option<Checksum> {
        self.sha512
            .as_deref()
            .and_then(Checksum::sha512)
            .or_else(|| self.sha1.as_deref().and_then(Checksum::sha1))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentVersion {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: Option<String>,
    pub version_type: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date_published: OffsetDateTime,
    pub dependencies: Vec<ContentDependency>,
    pub files: Vec<ContentFileInfo>,
}

impl ContentVersion {
    pub fn primary_file(&self) -> Option<&ContentFileInfo> {
        self.files.first()
    }
}

pub trait ContentCatalog: Send + Sync + 'static {
    fn search<'a>(
        &'a self,
        query: &'a str,
        filters: &'a SearchFilters,
    ) -> BoxFut<'a, Result<ContentSearchResult, AppError>>;

    fn project<'a>(&'a self, id_or_slug: &'a str) -> BoxFut<'a, Result<ContentProject, AppError>>;

    fn versions<'a>(
        &'a self,
        project: &'a str,
        loader: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<ContentVersion>, AppError>>;

    fn version<'a>(&'a self, id: &'a str) -> BoxFut<'a, Result<ContentVersion, AppError>>;

    fn download<'a>(
        &'a self,
        file: &'a ContentFileInfo,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>>;
}

/// Nguồn Modrinth thật: tra API và tải file qua `HttpClient` (có cache theo hash).
#[derive(Debug, Clone)]
pub struct ModrinthCatalog {
    http: Arc<HttpClient>,
    modrinth: Modrinth,
    hosts: HostAllowlist,
}

impl ModrinthCatalog {
    pub fn new(http: Arc<HttpClient>) -> Self {
        let modrinth = Modrinth::new(Arc::clone(&http));
        Self {
            http,
            modrinth,
            hosts: hosts::MODRINTH,
        }
    }

    pub fn with_modrinth(mut self, modrinth: Modrinth, hosts: HostAllowlist) -> Self {
        self.modrinth = modrinth;
        self.hosts = hosts;
        self
    }
}

impl ContentCatalog for ModrinthCatalog {
    fn search<'a>(
        &'a self,
        query: &'a str,
        filters: &'a SearchFilters,
    ) -> BoxFut<'a, Result<ContentSearchResult, AppError>> {
        Box::pin(async move {
            let res = self.modrinth.search(query, filters).await?;
            Ok(ContentSearchResult {
                hits: res.hits.into_iter().map(modrinth_hit).collect(),
                offset: res.offset,
                limit: res.limit,
                total_hits: res.total_hits,
            })
        })
    }

    fn project<'a>(&'a self, id_or_slug: &'a str) -> BoxFut<'a, Result<ContentProject, AppError>> {
        Box::pin(async move { Ok(modrinth_project(self.modrinth.project(id_or_slug).await?)) })
    }

    fn versions<'a>(
        &'a self,
        project: &'a str,
        loader: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<ContentVersion>, AppError>> {
        Box::pin(async move {
            let list = self.modrinth.versions(project, loader, minecraft).await?;
            Ok(list.into_iter().map(modrinth_version).collect())
        })
    }

    fn version<'a>(&'a self, id: &'a str) -> BoxFut<'a, Result<ContentVersion, AppError>> {
        Box::pin(async move { Ok(modrinth_version(self.modrinth.version(id).await?)) })
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

/// Nguồn mặc định khi tạo `App::with_deps` trong test offline.
#[derive(Debug, Default)]
pub struct NoContentCatalog;

impl NoContentCatalog {
    fn offline() -> AppError {
        AppError::new(ErrorCode::NetworkUnavailable, "chưa cấu hình nguồn mod")
    }
}

impl ContentCatalog for NoContentCatalog {
    fn search<'a>(
        &'a self,
        _: &'a str,
        _: &'a SearchFilters,
    ) -> BoxFut<'a, Result<ContentSearchResult, AppError>> {
        Box::pin(async { Err(Self::offline()) })
    }

    fn project<'a>(&'a self, _: &'a str) -> BoxFut<'a, Result<ContentProject, AppError>> {
        Box::pin(async { Err(Self::offline()) })
    }

    fn versions<'a>(
        &'a self,
        _: &'a str,
        _: LoaderKind,
        _: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<ContentVersion>, AppError>> {
        Box::pin(async { Err(Self::offline()) })
    }

    fn version<'a>(&'a self, _: &'a str) -> BoxFut<'a, Result<ContentVersion, AppError>> {
        Box::pin(async { Err(Self::offline()) })
    }

    fn download<'a>(
        &'a self,
        _: &'a ContentFileInfo,
        _: &'a Path,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async { Err(Self::offline()) })
    }
}

pub(crate) fn download_with_hosts<'a>(
    http: &'a HttpClient,
    hosts: &'a HostAllowlist,
    file: &'a ContentFileInfo,
    dest: &'a Path,
    progress: ProgressFn<'a>,
    cancel: &'a CancellationToken,
) -> BoxFut<'a, Result<(), AppError>> {
    Box::pin(async move {
        let url = file.url.as_deref().ok_or_else(|| {
            AppError::new(
                ErrorCode::ContentIncompatible,
                format!("file `{}` không có link tải trực tiếp", file.filename),
            )
        })?;
        let checksum: Checksum = file.checksum().ok_or_else(|| {
            AppError::new(
                ErrorCode::ContentIncompatible,
                format!("file `{}` thiếu checksum SHA-512/SHA-1", file.filename),
            )
        })?;
        let req = Download {
            url,
            checksum: &checksum,
            dest,
            size: Some(file.size),
            allowed_hosts: Some(hosts),
        };
        http.download(req, progress, cancel).await?;
        Ok(())
    })
}

fn modrinth_hit(h: SearchHit) -> ContentSearchHit {
    ContentSearchHit {
        source: CatalogSource::Modrinth,
        project_id: h.project_id,
        slug: h.slug,
        project_type: h.project_type,
        title: h.title,
        description: h.description,
        icon_url: h.icon_url,
        downloads: h.downloads,
        categories: h.categories,
        game_versions: h.versions,
        server_side: Some(h.server_side),
    }
}

fn modrinth_project(p: Project) -> ContentProject {
    let page_url = p.page_url();
    ContentProject {
        source: CatalogSource::Modrinth,
        id: p.id,
        slug: p.slug,
        project_type: p.project_type,
        title: p.title,
        description: p.description,
        body: Some(p.body),
        icon_url: p.icon_url,
        downloads: p.downloads,
        loaders: p.loaders,
        game_versions: p.game_versions,
        client_side: Some(p.client_side),
        server_side: Some(p.server_side),
        page_url,
        links: p.links,
    }
}

fn modrinth_version(v: ModrinthVersion) -> ContentVersion {
    let primary_idx = v.files.iter().position(|f| f.primary).unwrap_or(0);
    let mut files: Vec<ContentFileInfo> = v.files.into_iter().map(modrinth_file).collect();
    if primary_idx < files.len() {
        files.swap(0, primary_idx);
    }
    ContentVersion {
        id: v.id,
        project_id: v.project_id,
        name: v.name,
        version_number: Some(v.version_number),
        version_type: v.version_type,
        game_versions: v.game_versions,
        loaders: v.loaders,
        date_published: v.date_published,
        dependencies: v
            .dependencies
            .into_iter()
            .map(|d| ContentDependency {
                project_id: d.project_id,
                version_id: d.version_id,
                dependency_type: d.dependency_type,
            })
            .collect(),
        files,
    }
}

fn modrinth_file(f: VersionFile) -> ContentFileInfo {
    ContentFileInfo {
        url: Some(f.url),
        filename: f.filename,
        size: f.size,
        sha1: Some(f.hashes.sha1),
        sha512: Some(f.hashes.sha512),
        manual_download: false,
        download_page_url: None,
    }
}
