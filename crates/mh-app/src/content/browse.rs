//! Duyệt catalog: tìm kiếm, chi tiết và phiên bản của một project (spec trình duyệt mod, mục 3, 5).

use mh_core::{ContentKind, ErrorCode, LoaderKind, McVersion, ServerId};

use super::catalog::{
    CatalogSource, ContentProject, ContentSearchResult, ContentVersion, ProjectType, SearchFilters,
    SearchIndex,
};
use super::{CONTENT_PAGE_SIZE, content_kind_for, plan};
use crate::{App, AppError};

/// Loader và Minecraft để lọc catalog khi chưa có server (bước Mod lúc tạo server).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentTarget {
    pub loader: LoaderKind,
    pub minecraft: McVersion,
}

impl ContentTarget {
    fn kind(&self) -> Result<ContentKind, AppError> {
        self.loader
            .content_kind()
            .ok_or_else(|| plan::incompatible(self.loader.as_str()))
    }
}

/// Điều kiện tìm kiếm nội dung từ giao diện.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentSearch {
    pub kind: Option<ProjectType>,
    /// Lọc theo loader, Minecraft của server và bỏ mod chỉ chạy ở client.
    pub server_id: Option<ServerId>,
    /// Như `server_id` nhưng cho server chưa tạo; bỏ qua khi đã có `server_id`.
    pub target: Option<ContentTarget>,
    /// Trang, từ 1.
    pub page: u32,
    pub index: SearchIndex,
}

impl Default for ContentSearch {
    fn default() -> Self {
        Self {
            kind: None,
            server_id: None,
            target: None,
            page: 1,
            index: SearchIndex::Relevance,
        }
    }
}

impl App {
    /// Tìm mod/plugin/modpack trên catalog. Có server hoặc `target` thì lọc theo loader, Minecraft
    /// và chỉ lấy mục chạy được phía server (`server_side != unsupported` với Modrinth).
    pub async fn search_content(
        &self,
        source: CatalogSource,
        query: &str,
        search: &ContentSearch,
    ) -> Result<ContentSearchResult, AppError> {
        if search.page == 0 {
            return Err(AppError::new(ErrorCode::InvalidInput, "page phải >= 1")
                .with_params(serde_json::json!({ "field": "page" })));
        }
        let mut filters = SearchFilters {
            project_type: search.kind,
            limit: CONTENT_PAGE_SIZE,
            offset: search
                .page
                .saturating_sub(1)
                .saturating_mul(CONTENT_PAGE_SIZE),
            index: search.index,
            ..SearchFilters::default()
        };
        let target = match search.server_id {
            Some(id) => Some(self.content_target(id)?),
            None => search.target.clone(),
        };
        if let Some(target) = target {
            let kind = target.kind()?;
            if filters.project_type.is_none() {
                filters.project_type = Some(match kind {
                    ContentKind::Mods => ProjectType::Mod,
                    ContentKind::Plugins => ProjectType::Plugin,
                });
            }
            filters.loader = Some(target.loader);
            filters.minecraft = Some(target.minecraft);
            filters.server_only = true;
        }
        self.catalog_for(source)?.search(query, &filters).await
    }

    /// Chi tiết một project, kèm mô tả đầy đủ khi nguồn trả riêng (CurseForge).
    pub async fn content_project(
        &self,
        source: CatalogSource,
        project_id: &str,
    ) -> Result<ContentProject, AppError> {
        let catalog = self.catalog_for(source)?;
        let mut project = catalog.project(project_id).await?;
        if project.body.is_none() {
            // Không có mô tả đầy đủ thì vẫn hiện được phần còn lại: bỏ qua lỗi của lời gọi phụ.
            project.body = catalog.project_body(project_id).await.ok().flatten();
        }
        Ok(project)
    }

    /// Các phiên bản của project hợp với loader và Minecraft của server, mới nhất trước.
    pub async fn content_versions(
        &self,
        source: CatalogSource,
        project_id: &str,
        server_id: ServerId,
    ) -> Result<Vec<ContentVersion>, AppError> {
        let target = self.content_target(server_id)?;
        self.target_versions(source, project_id, &target).await
    }

    /// Như `content_versions` cho server chưa tạo.
    pub async fn target_versions(
        &self,
        source: CatalogSource,
        project_id: &str,
        target: &ContentTarget,
    ) -> Result<Vec<ContentVersion>, AppError> {
        let catalog = self.catalog_for(source)?;
        let _ = target.kind()?;
        catalog
            .versions(project_id, target.loader, &target.minecraft)
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

    fn content_target(&self, id: ServerId) -> Result<ContentTarget, AppError> {
        let spec = self.store.load(id)?;
        let _ = content_kind_for(&spec)?;
        Ok(ContentTarget {
            loader: spec.game.loader,
            minecraft: spec.game.minecraft,
        })
    }
}
