//! Quản lý mod/plugin: đọc danh sách, lập kế hoạch cài và tải từ Modrinth / CurseForge.

mod catalog;
mod curseforge;
mod install;
mod manage;
mod modrinth;
mod pending_upload;
mod plan;
mod preflight;
mod requests;
mod running;
mod updates;

use std::collections::BTreeSet;
use std::sync::Arc;

use mh_core::{ContentKind, ErrorCode, InstanceSpec, ServerId};
use mh_store::ContentManifest;
pub use mh_store::{ContentFile, ContentSource, PendingContent, ProjectId};

pub use catalog::{
    BlockedPackFile, CatalogSource, CfResolvedFiles, ContentCatalog, ContentDependency,
    ContentFileInfo, ContentProject, ContentSearchHit, ContentSearchResult, ContentVersion,
    DependencyType, Modrinth, NoContentCatalog, PackFile, ProjectLinks, ProjectType, SearchFilters,
    SearchIndex, SideSupport, modrinth_loaders,
};
pub(crate) use curseforge::parse_cf_id;
pub use curseforge::{CurseForge, CurseforgeCatalog};
pub use manage::PendingUploadResult;
pub use modrinth::ModrinthCatalog;
pub use plan::{ContentInstallPlan, PlannedContent};
pub use preflight::{
    ClientOnlyMod, MissingDependency, PreflightReport, VersionMismatch, WrongLoaderMod,
};
pub(crate) use preflight::{missing_server_deps, preflight_block_error, reads_mods_toml_deps};
pub use requests::{ContentRequest, MAX_INSTALL_ITEMS};
pub(crate) use running::AddedWhileRunning;
pub use updates::{MAX_UPDATE_ITEMS, UpdateCandidate, UpdateItem, UpdateReport};

use crate::files::blocking;
use crate::{App, AppError, lock};

/// Số kết quả mỗi trang tìm kiếm nội dung.
pub const CONTENT_PAGE_SIZE: u32 = 20;

/// Điều kiện tìm kiếm nội dung từ giao diện.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentSearch {
    pub kind: Option<ProjectType>,
    /// Lọc theo loader, Minecraft của server và bỏ mod chỉ chạy ở client.
    pub server_id: Option<ServerId>,
    /// Trang, từ 1.
    pub page: u32,
    pub index: SearchIndex,
}

impl Default for ContentSearch {
    fn default() -> Self {
        Self {
            kind: None,
            server_id: None,
            page: 1,
            index: SearchIndex::Relevance,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContentList {
    pub kind: ContentKind,
    pub files: Vec<ContentFile>,
    pub pending: Vec<PendingContent>,
    /// File thêm vào lúc process hiện tại đang chạy: có hiệu lực sau khi khởi động lại.
    pub needs_restart: BTreeSet<String>,
}

impl App {
    pub fn with_content(mut self, catalog: Arc<dyn ContentCatalog>) -> Self {
        self.content_catalog = catalog;
        self
    }

    pub fn with_curseforge(mut self, catalog: Arc<dyn ContentCatalog>) -> Self {
        self.curseforge_catalog = Some(catalog);
        self
    }

    pub(crate) fn catalog_for(
        &self,
        source: CatalogSource,
    ) -> Result<Arc<dyn ContentCatalog>, AppError> {
        match source {
            CatalogSource::Modrinth => Ok(Arc::clone(&self.content_catalog)),
            CatalogSource::Curseforge => {
                let key = self.effective_curseforge_key()?.ok_or_else(|| {
                    AppError::new(
                        ErrorCode::CurseforgeUnavailable,
                        "chưa có CurseForge API key",
                    )
                })?;
                if let Some(custom) = &self.curseforge_catalog {
                    return Ok(Arc::clone(custom));
                }
                match &self.http {
                    Some(http) => Ok(Arc::new(CurseforgeCatalog::new(
                        Arc::clone(http),
                        key.expose(),
                    ))),
                    None => Ok(Arc::new(NoContentCatalog)),
                }
            }
        }
    }

    /// Tìm mod/plugin/modpack trên catalog. Có `server_id` thì lọc theo loader, Minecraft và
    /// chỉ lấy mục chạy được phía server (`server_side != unsupported` với Modrinth).
    pub async fn search_content(
        &self,
        source: CatalogSource,
        query: &str,
        search: &ContentSearch,
    ) -> Result<ContentSearchResult, AppError> {
        let ContentSearch {
            kind,
            server_id,
            page,
            index,
        } = *search;
        if page == 0 {
            return Err(AppError::new(ErrorCode::InvalidInput, "page phải >= 1")
                .with_params(serde_json::json!({ "field": "page" })));
        }
        let mut filters = SearchFilters {
            project_type: kind,
            limit: CONTENT_PAGE_SIZE,
            offset: page.saturating_sub(1).saturating_mul(CONTENT_PAGE_SIZE),
            index,
            ..SearchFilters::default()
        };
        if let Some(id) = server_id {
            let spec = self.store.load(id)?;
            let server_kind = content_kind_for(&spec)?;
            if filters.project_type.is_none() {
                filters.project_type = Some(match server_kind {
                    ContentKind::Mods => ProjectType::Mod,
                    ContentKind::Plugins => ProjectType::Plugin,
                });
            }
            filters.loader = Some(spec.game.loader);
            filters.minecraft = Some(spec.game.minecraft);
            filters.server_only = true;
        }
        self.catalog_for(source)?.search(query, &filters).await
    }

    /// Chi tiết một project từ nguồn nội dung.
    pub async fn content_project(
        &self,
        source: CatalogSource,
        project_id: &str,
    ) -> Result<ContentProject, AppError> {
        self.catalog_for(source)?.project(project_id).await
    }

    /// Các phiên bản của project hợp với loader và Minecraft của server, mới nhất trước.
    pub async fn content_versions(
        &self,
        source: CatalogSource,
        project_id: &str,
        server_id: ServerId,
    ) -> Result<Vec<ContentVersion>, AppError> {
        let catalog = self.catalog_for(source)?;
        let spec = self.store.load(server_id)?;
        let _ = content_kind_for(&spec)?;
        catalog
            .versions(project_id, spec.game.loader, &spec.game.minecraft)
            .await
    }

    /// Mọi phiên bản của project khi chưa có server (dùng cho modpack), mới nhất trước.
    pub async fn project_versions(
        &self,
        source: CatalogSource,
        project_id: &str,
    ) -> Result<Vec<ContentVersion>, AppError> {
        self.catalog_for(source)?.project_versions(project_id).await
    }

    /// Danh sách mod/plugin hiện trên đĩa cùng các mục `pending`. Vanilla trả `CONTENT_INCOMPATIBLE`.
    pub async fn content(&self, id: ServerId) -> Result<ContentList, AppError> {
        let spec = self.store.load(id)?;
        let kind = content_kind_for(&spec)?;
        let mutex = self.content_lock(id);
        let _guard = mutex.lock().await;
        let (store, scanner) = (self.store.clone(), Arc::clone(&self.content_scanner));
        let needs_restart = self.content_needs_restart(id);
        blocking(move || {
            let paths = store.paths(id);
            let files = scanner.scan_dirs(&paths.server_dir, &paths.content_file, kind)?;
            let manifest = ContentManifest::load(&paths.content_file)?;
            Ok(ContentList {
                kind,
                files,
                pending: manifest.pending,
                needs_restart,
            })
        })
        .await
    }

    pub(crate) fn content_lock(&self, id: ServerId) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = lock(&self.content_locks);
        Arc::clone(
            locks
                .entry(id)
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))),
        )
    }

    pub(crate) fn ensure_content_idle(
        &self,
        id: ServerId,
    ) -> Result<(InstanceSpec, ContentKind), AppError> {
        let view = self.get(id)?;
        if view.installing.is_some()
            || view.state.has_process()
            || view.status.as_ref().is_some_and(|s| s.restart_pending)
        {
            return Err(AppError::new(
                ErrorCode::ServerBusy,
                "không sửa mod khi server đang chạy",
            ));
        }
        let spec = view
            .spec
            .ok_or_else(|| AppError::new(ErrorCode::ConfigInvalid, "server hỏng cấu hình"))?;
        let kind = content_kind_for(&spec)?;
        Ok((spec, kind))
    }
}

pub(super) fn content_kind_for(spec: &InstanceSpec) -> Result<ContentKind, AppError> {
    spec.game
        .loader
        .content_kind()
        .ok_or_else(|| plan::incompatible(spec.game.loader.as_str()))
}
