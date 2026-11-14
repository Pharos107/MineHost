//! Quản lý mod/plugin: đọc danh sách, lập kế hoạch cài và tải từ Modrinth.

mod catalog;
mod install;
mod manage;
mod plan;
mod preflight;

use std::sync::Arc;

use mh_core::{ContentKind, ErrorCode, InstanceSpec, ServerId};
use mh_store::ContentManifest;
pub use mh_store::{ContentFile, ContentSource, PendingContent, ProjectId};

pub use catalog::{
    ContentCatalog, Dependency, DependencyType, FileHashes, Modrinth, ModrinthCatalog,
    ModrinthVersion, NoContentCatalog, PackFile, Project, ProjectLinks, ProjectType, SearchFilters,
    SearchHit, SearchIndex, SearchResult, SideSupport, VersionFile, modrinth_loaders,
};
pub use plan::{ContentInstallPlan, PlannedContent};
pub(crate) use preflight::preflight_block_error;
pub use preflight::{
    ClientOnlyMod, MissingDependency, PreflightReport, VersionMismatch, WrongLoaderMod,
};

use crate::files::blocking;
use crate::{App, AppError, lock};

/// Số kết quả mỗi trang tìm kiếm nội dung.
pub const CONTENT_PAGE_SIZE: u32 = 20;

#[derive(Debug, Clone, PartialEq)]
pub struct ContentList {
    pub kind: ContentKind,
    pub files: Vec<ContentFile>,
    pub pending: Vec<PendingContent>,
}

impl App {
    pub fn with_content(mut self, catalog: Arc<dyn ContentCatalog>) -> Self {
        self.content_catalog = catalog;
        self
    }

    /// Tìm mod/plugin/modpack trên catalog. Có `server_id` thì lọc theo loader, Minecraft và
    /// chỉ lấy mục chạy được phía server (`server_side != unsupported`).
    pub async fn search_content(
        &self,
        query: &str,
        kind: Option<ProjectType>,
        server_id: Option<ServerId>,
        page: u32,
    ) -> Result<SearchResult, AppError> {
        if page == 0 {
            return Err(AppError::new(ErrorCode::InvalidInput, "page phải >= 1")
                .with_params(serde_json::json!({ "field": "page" })));
        }
        let mut filters = SearchFilters {
            project_type: kind,
            limit: CONTENT_PAGE_SIZE,
            offset: page.saturating_sub(1).saturating_mul(CONTENT_PAGE_SIZE),
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
        self.content_catalog.search(query, &filters).await
    }

    /// Chi tiết một project từ nguồn nội dung.
    pub async fn content_project(&self, project_id: &str) -> Result<Project, AppError> {
        self.content_catalog.project(project_id).await
    }

    /// Các phiên bản của project hợp với loader và Minecraft của server, mới nhất trước.
    pub async fn content_versions(
        &self,
        project_id: &str,
        server_id: ServerId,
    ) -> Result<Vec<ModrinthVersion>, AppError> {
        let spec = self.store.load(server_id)?;
        let _ = content_kind_for(&spec)?;
        self.content_catalog
            .versions(project_id, spec.game.loader, &spec.game.minecraft)
            .await
    }

    /// Kiểm tra nhanh server có tồn tại, hỗ trợ mod/plugin và đang rảnh trước khi nhận upload.
    pub fn check_content_idle(&self, id: ServerId) -> Result<(), AppError> {
        self.ensure_content_idle(id).map(|_| ())
    }

    /// Danh sách mod/plugin hiện trên đĩa cùng các mục `pending`. Vanilla trả `CONTENT_INCOMPATIBLE`.
    pub async fn content(&self, id: ServerId) -> Result<ContentList, AppError> {
        let spec = self.store.load(id)?;
        let kind = content_kind_for(&spec)?;
        let mutex = self.content_lock(id);
        let _guard = mutex.lock().await;
        let (store, scanner) = (self.store.clone(), Arc::clone(&self.content_scanner));
        blocking(move || {
            let paths = store.paths(id);
            let files = scanner.scan_dirs(&paths.server_dir, &paths.content_file, kind)?;
            let manifest = ContentManifest::load(&paths.content_file)?;
            Ok(ContentList {
                kind,
                files,
                pending: manifest.pending,
            })
        })
        .await
    }

    /// Lập kế hoạch cài đặt (mod chính + dependency bắt buộc), chưa tải file.
    pub async fn plan_content_install(
        &self,
        id: ServerId,
        project_id: &str,
        version_id: Option<&str>,
    ) -> Result<ContentInstallPlan, AppError> {
        let spec = self.store.load(id)?;
        let kind = content_kind_for(&spec)?;
        let installed = {
            let mutex = self.content_lock(id);
            let _guard = mutex.lock().await;
            let (store, scanner) = (self.store.clone(), Arc::clone(&self.content_scanner));
            blocking(move || {
                let paths = store.paths(id);
                let files = scanner.scan_dirs(&paths.server_dir, &paths.content_file, kind)?;
                Ok(plan::installed_projects(&files))
            })
            .await?
        };
        plan::build_plan(
            self.content_catalog.as_ref(),
            kind,
            spec.game.loader,
            &spec.game.minecraft,
            &installed,
            project_id,
            version_id,
        )
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

fn content_kind_for(spec: &InstanceSpec) -> Result<ContentKind, AppError> {
    spec.game
        .loader
        .content_kind()
        .ok_or_else(|| plan::incompatible(spec.game.loader.as_str()))
}
