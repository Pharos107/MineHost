//! Nguồn danh mục mod/plugin dùng chung cho Modrinth và CurseForge.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use mh_core::{CfManifestFile, ErrorCode, LoaderKind, McVersion};
use mh_net::{Checksum, Download, HostAllowlist, HttpClient, hosts};
pub use mh_providers::modpack::{BlockedPackFile, PackFile};
pub use mh_providers::modrinth::{
    DependencyType, Modrinth, ProjectLinks, ProjectType, SearchFilters, SearchIndex, SideSupport,
    modrinth_loaders,
};
use mh_store::{ContentSource, ProjectId};
use time::OffsetDateTime;

use super::curseforge::parse_cf_id;
use crate::AppError;
use crate::sources::{BoxFut, CancellationToken, ProgressFn};

pub type CfResolvedFiles = (Vec<PackFile>, Vec<BlockedPackFile>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatalogSource {
    Modrinth,
    Curseforge,
}

impl serde::Serialize for CatalogSource {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
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

    /// Bản ghi có ID project của nguồn này không. Mod cài theo modpack mang `source = "modpack"`
    /// nhưng ID vẫn là của Modrinth (chuỗi) hoặc CurseForge (số); bỏ sót chúng thì cài thêm mod sẽ
    /// tải trùng dependency mà pack đã có.
    pub fn owns_record(self, source: ContentSource, project_id: Option<&ProjectId>) -> bool {
        if source == self.to_content_source() {
            return true;
        }
        source == ContentSource::Modpack
            && matches!(
                (self, project_id),
                (Self::Modrinth, Some(ProjectId::Str(_)))
                    | (Self::Curseforge, Some(ProjectId::Int(_)))
            )
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

    fn project_versions<'a>(
        &'a self,
        project: &'a str,
    ) -> BoxFut<'a, Result<Vec<ContentVersion>, AppError>> {
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

    /// Mô tả đầy đủ khi `project` không kèm sẵn (CurseForge trả riêng, dạng HTML).
    fn project_body<'a>(&'a self, id: &'a str) -> BoxFut<'a, Result<Option<String>, AppError>> {
        let _ = id;
        Box::pin(async { Ok(None) })
    }

    /// Tra phiên bản theo SHA-1 của file (Modrinth `version_files`), để nhận ra mod "Thêm tay" và
    /// kiểm tra cập nhật (spec P3, mục 5.7). Hash không có trên nguồn thì vắng mặt trong map. Nguồn
    /// không hỗ trợ (CurseForge dùng fingerprint riêng) trả map rỗng.
    fn versions_by_hash<'a>(
        &'a self,
        sha1: &'a [String],
    ) -> BoxFut<'a, Result<HashMap<String, ContentVersion>, AppError>> {
        let _ = sha1;
        Box::pin(async { Ok(HashMap::new()) })
    }

    /// Các SHA-1 trong `sha1` mà project chứa file đó ghi `server_side = unsupported` trên nguồn
    /// (spec P3, mục 3.5, nguồn 4). Nguồn không hỗ trợ trả tập rỗng.
    fn server_unsupported_by_hash<'a>(
        &'a self,
        sha1: &'a [String],
    ) -> BoxFut<'a, Result<HashSet<String>, AppError>> {
        let _ = sha1;
        Box::pin(async { Ok(HashSet::new()) })
    }

    fn curseforge_server_pack<'a>(
        &'a self,
        file_id: &'a str,
    ) -> BoxFut<'a, Result<Option<ContentFileInfo>, AppError>> {
        let _ = file_id;
        Box::pin(async { Ok(None) })
    }

    fn resolve_curseforge_files<'a>(
        &'a self,
        files: &'a [CfManifestFile],
    ) -> BoxFut<'a, Result<CfResolvedFiles, AppError>> {
        if files.is_empty() {
            return Box::pin(async { Ok((Vec::new(), Vec::new())) });
        }
        Box::pin(async {
            Err(AppError::new(
                ErrorCode::CurseforgeUnavailable,
                "nguồn hiện tại không hỗ trợ giải quyết danh sách file CurseForge",
            ))
        })
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

pub(crate) fn download_pack_file_with_hosts<'a>(
    http: &'a HttpClient,
    hosts: &'a HostAllowlist,
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
                allowed_hosts: Some(hosts),
            };
            match http.download(req, &mut *progress, cancel).await {
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
