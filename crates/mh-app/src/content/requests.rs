//! Lập kế hoạch cho nhiều mục cùng lúc (danh sách chờ cài, spec trình duyệt mod mục 4).

use std::sync::Arc;

use mh_core::{ErrorCode, ServerId};

use super::catalog::CatalogSource;
use super::content_kind_for;
use super::plan::{self, ContentInstallPlan};
use crate::files::blocking;
use crate::{App, AppError};

/// Số mục tối đa trong một lần cài.
pub const MAX_INSTALL_ITEMS: usize = 50;

/// Một mục người dùng chọn cài. Không có `version_id` thì dùng bản mới nhất hợp với server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentRequest {
    pub source: CatalogSource,
    pub project_id: String,
    pub version_id: Option<String>,
}

impl App {
    /// Lập kế hoạch cài đặt một mục (mod chính + dependency bắt buộc), chưa tải file.
    pub async fn plan_content_install(
        &self,
        id: ServerId,
        source: CatalogSource,
        project_id: &str,
        version_id: Option<&str>,
    ) -> Result<ContentInstallPlan, AppError> {
        let request = ContentRequest {
            source,
            project_id: project_id.to_owned(),
            version_id: version_id.map(str::to_owned),
        };
        self.plan_content_requests(id, &[request])
            .await?
            .pop()
            .ok_or_else(|| AppError::new(ErrorCode::Internal, "kế hoạch rỗng"))
    }

    /// Lập kế hoạch cho nhiều mục: mỗi nguồn một kế hoạch, theo thứ tự nguồn xuất hiện lần đầu.
    /// Dependency dùng chung chỉ có một dòng.
    pub async fn plan_content_requests(
        &self,
        id: ServerId,
        requests: &[ContentRequest],
    ) -> Result<Vec<ContentInstallPlan>, AppError> {
        check_count(requests.len())?;
        let spec = self.store.load(id)?;
        let kind = content_kind_for(&spec)?;
        let files = {
            let mutex = self.content_lock(id);
            let _guard = mutex.lock().await;
            let (store, scanner) = (self.store.clone(), Arc::clone(&self.content_scanner));
            blocking(move || {
                let paths = store.paths(id);
                Ok(scanner.scan_dirs(&paths.server_dir, &paths.content_file, kind)?)
            })
            .await?
        };
        let mut plans = Vec::new();
        for (source, roots) in group_by_source(requests) {
            let catalog = self.catalog_for(source)?;
            let installed = plan::installed_projects(&files, source);
            let built = plan::build_plan(
                source,
                catalog.as_ref(),
                kind,
                spec.game.loader,
                &spec.game.minecraft,
                &installed,
                &roots,
            )
            .await?;
            plans.push(built);
        }
        Ok(plans)
    }
}

pub(super) fn check_count(n: usize) -> Result<(), AppError> {
    if n == 0 || n > MAX_INSTALL_ITEMS {
        return Err(AppError::new(ErrorCode::InvalidInput, "cần 1 đến 50 mục")
            .with_params(serde_json::json!({ "field": "items" })));
    }
    Ok(())
}

type Roots = Vec<(String, Option<String>)>;

fn group_by_source(requests: &[ContentRequest]) -> Vec<(CatalogSource, Roots)> {
    let mut groups: Vec<(CatalogSource, Roots)> = Vec::new();
    for req in requests {
        let root = (req.project_id.clone(), req.version_id.clone());
        match groups.iter_mut().find(|(s, _)| *s == req.source) {
            Some((_, roots)) => roots.push(root),
            None => groups.push((req.source, vec![root])),
        }
    }
    groups
}
