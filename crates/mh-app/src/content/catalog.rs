//! Nguồn danh mục mod/plugin (Modrinth) tách trait để test không cần mạng.

use std::path::Path;
use std::sync::Arc;

use mh_core::{ErrorCode, LoaderKind, McVersion};
use mh_net::{Checksum, Download, HostAllowlist, HttpClient, hosts};
pub use mh_providers::modpack::PackFile;
pub use mh_providers::modrinth::{
    Dependency, DependencyType, FileHashes, Modrinth, ModrinthVersion, Project, ProjectLinks,
    ProjectType, SearchFilters, SearchHit, SearchIndex, SearchResult, SideSupport, VersionFile,
    modrinth_loaders,
};

use crate::AppError;
use crate::sources::{BoxFut, CancellationToken, ProgressFn};

pub trait ContentCatalog: Send + Sync + 'static {
    fn search<'a>(
        &'a self,
        query: &'a str,
        filters: &'a SearchFilters,
    ) -> BoxFut<'a, Result<SearchResult, AppError>>;

    fn project<'a>(&'a self, id_or_slug: &'a str) -> BoxFut<'a, Result<Project, AppError>>;

    fn versions<'a>(
        &'a self,
        project: &'a str,
        loader: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<ModrinthVersion>, AppError>>;

    fn version<'a>(&'a self, id: &'a str) -> BoxFut<'a, Result<ModrinthVersion, AppError>>;

    fn download<'a>(
        &'a self,
        file: &'a VersionFile,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>>;

    fn project_versions<'a>(
        &'a self,
        project: &'a str,
    ) -> BoxFut<'a, Result<Vec<ModrinthVersion>, AppError>> {
        let _ = project;
        Box::pin(async {
            Err(AppError::new(
                ErrorCode::NetworkUnavailable,
                "chưa cấu hình nguồn danh sách phiên bản modpack",
            ))
        })
    }

    fn download_pack_file<'a>(
        &'a self,
        file: &'a PackFile,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        let _ = (file, dest, progress, cancel);
        Box::pin(async {
            Err(AppError::new(
                ErrorCode::NetworkUnavailable,
                "chưa cấu hình nguồn tải file modpack",
            ))
        })
    }

    fn pack_hosts(&self) -> HostAllowlist {
        hosts::MODRINTH
    }
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
    ) -> BoxFut<'a, Result<SearchResult, AppError>> {
        Box::pin(async move { Ok(self.modrinth.search(query, filters).await?) })
    }

    fn project<'a>(&'a self, id_or_slug: &'a str) -> BoxFut<'a, Result<Project, AppError>> {
        Box::pin(async move { Ok(self.modrinth.project(id_or_slug).await?) })
    }

    fn versions<'a>(
        &'a self,
        project: &'a str,
        loader: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<ModrinthVersion>, AppError>> {
        Box::pin(async move { Ok(self.modrinth.versions(project, loader, minecraft).await?) })
    }

    fn version<'a>(&'a self, id: &'a str) -> BoxFut<'a, Result<ModrinthVersion, AppError>> {
        Box::pin(async move { Ok(self.modrinth.version(id).await?) })
    }

    fn download<'a>(
        &'a self,
        file: &'a VersionFile,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            let checksum: Checksum = file.checksum().ok_or_else(|| {
                AppError::new(
                    ErrorCode::ContentIncompatible,
                    format!("file `{}` thiếu checksum SHA-512/SHA-1", file.filename),
                )
            })?;
            let req = Download {
                url: &file.url,
                checksum: &checksum,
                dest,
                size: Some(file.size),
                allowed_hosts: Some(&self.hosts),
            };
            self.http.download(req, progress, cancel).await?;
            Ok(())
        })
    }

    fn project_versions<'a>(
        &'a self,
        project: &'a str,
    ) -> BoxFut<'a, Result<Vec<ModrinthVersion>, AppError>> {
        Box::pin(async move { Ok(self.modrinth.project_versions(project).await?) })
    }

    fn download_pack_file<'a>(
        &'a self,
        file: &'a PackFile,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            let mut last_err = AppError::new(
                ErrorCode::ModpackInvalid,
                format!("file `{}` trong modpack không có URL tải", file.path),
            );
            for url in &file.urls {
                let req = Download {
                    url,
                    checksum: &file.checksum,
                    dest,
                    size: Some(file.size),
                    allowed_hosts: Some(&self.hosts),
                };
                match self.http.download(req, &mut *progress, cancel).await {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        last_err = AppError::from(e);
                        if cancel.is_cancelled() {
                            return Err(last_err);
                        }
                    }
                }
            }
            Err(last_err)
        })
    }

    fn pack_hosts(&self) -> HostAllowlist {
        self.hosts
    }
}

/// Nguồn mặc định khi tạo `App::with_deps` trong test offline.
#[derive(Debug, Default)]
pub struct NoContentCatalog;

impl NoContentCatalog {
    fn offline() -> AppError {
        AppError::new(
            ErrorCode::NetworkUnavailable,
            "chưa cấu hình nguồn mod Modrinth",
        )
    }
}

impl ContentCatalog for NoContentCatalog {
    fn search<'a>(
        &'a self,
        _: &'a str,
        _: &'a SearchFilters,
    ) -> BoxFut<'a, Result<SearchResult, AppError>> {
        Box::pin(async { Err(Self::offline()) })
    }

    fn project<'a>(&'a self, _: &'a str) -> BoxFut<'a, Result<Project, AppError>> {
        Box::pin(async { Err(Self::offline()) })
    }

    fn versions<'a>(
        &'a self,
        _: &'a str,
        _: LoaderKind,
        _: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<ModrinthVersion>, AppError>> {
        Box::pin(async { Err(Self::offline()) })
    }

    fn version<'a>(&'a self, _: &'a str) -> BoxFut<'a, Result<ModrinthVersion, AppError>> {
        Box::pin(async { Err(Self::offline()) })
    }

    fn download<'a>(
        &'a self,
        _: &'a VersionFile,
        _: &'a Path,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async { Err(Self::offline()) })
    }
}
