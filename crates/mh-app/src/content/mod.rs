//! Quản lý mod/plugin: đọc danh sách, lập kế hoạch cài và tải từ Modrinth.

mod catalog;
mod install;
mod plan;

use std::sync::Arc;

use mh_core::{ContentKind, ErrorCode, InstanceSpec, ServerId};
use mh_store::{ContentFile, ContentManifest, PendingContent};

pub use catalog::{
    ContentCatalog, Dependency, DependencyType, FileHashes, Modrinth, ModrinthCatalog,
    ModrinthVersion, NoContentCatalog, Project, SideSupport, VersionFile,
};
pub use plan::{ContentInstallPlan, PlannedContent};

use crate::files::blocking;
use crate::{App, AppError, lock};

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
