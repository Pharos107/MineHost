//! Nguồn danh mục Modrinth: tra API và tải file qua `HttpClient` (có cache theo hash).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use mh_core::{LoaderKind, McVersion};
use mh_net::{HostAllowlist, HttpClient, hosts};
use mh_providers::modpack::PackFile;
use mh_providers::modrinth::{
    Modrinth, ModrinthVersion, Project, SearchFilters, SearchHit, VersionFile,
};

use super::catalog::{
    CatalogSource, ContentCatalog, ContentDependency, ContentFileInfo, ContentProject,
    ContentSearchHit, ContentSearchResult, ContentVersion, download_pack_file_with_hosts,
    download_with_hosts,
};
use crate::AppError;
use crate::sources::{BoxFut, CancellationToken, ProgressFn};

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

    fn versions_by_hash<'a>(
        &'a self,
        sha1: &'a [String],
    ) -> BoxFut<'a, Result<HashMap<String, ContentVersion>, AppError>> {
        Box::pin(async move {
            let found = self.modrinth.versions_by_hash(sha1).await?;
            Ok(found
                .into_iter()
                .map(|(hash, ver)| (hash.to_ascii_lowercase(), modrinth_version(ver)))
                .collect())
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

    fn project_versions<'a>(
        &'a self,
        project: &'a str,
    ) -> BoxFut<'a, Result<Vec<ContentVersion>, AppError>> {
        Box::pin(async move {
            let list = self.modrinth.project_versions(project).await?;
            Ok(list.into_iter().map(modrinth_version).collect())
        })
    }

    fn download_pack_file<'a>(
        &'a self,
        file: &'a PackFile,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        download_pack_file_with_hosts(&self.http, &self.hosts, file, dest, progress, cancel)
    }

    fn pack_hosts(&self) -> HostAllowlist {
        self.hosts
    }
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
