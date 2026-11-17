//! Kiểm tra và cập nhật mod/plugin (spec P3, mục 3.9 và 5.7).
//!
//! Kiểm tra hỏi Modrinth theo SHA-1 của từng jar, nên nhận ra cả mod "Thêm tay". Mod CurseForge đã
//! có bản ghi thì hỏi theo project id. Mod thuộc modpack không nằm trong danh sách: chúng đi theo
//! lần cập nhật modpack.

use std::collections::HashMap;
use std::sync::Arc;

use mh_core::{ErrorCode, ServerId};
use mh_store::{ContentFile, ContentManifest, ContentRecord, ContentSource};
use serde::Serialize;
use time::OffsetDateTime;

use super::catalog::{CatalogSource, ContentCatalog, ContentVersion};
use crate::files::blocking;
use crate::jobs::{JobContext, JobKind, JobStep};
use crate::update::with_backup;
use crate::{App, AppError, JobId};

/// Giới hạn số mục trong một lần cập nhật, cùng cỡ với modpack lớn.
pub const MAX_UPDATE_ITEMS: usize = 500;

/// Một mod có bản mới hơn bản đang cài.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateCandidate {
    /// Đường dẫn dạng đang bật (`mods/x.jar`), dùng lại khi gửi yêu cầu cập nhật.
    pub file: String,
    pub source: CatalogSource,
    pub project_id: String,
    /// `None` khi chưa biết bản đang cài (mod thêm tay không có trên nguồn theo hash).
    pub current_version_id: Option<String>,
    pub latest_version_id: String,
    pub latest_version_name: String,
    /// Mod chưa có bản ghi, được nhận ra qua hash.
    pub manual: bool,
}

/// Kết quả Job kiểm tra cập nhật, nằm trong `result` của Job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct UpdateReport {
    pub updates: Vec<UpdateCandidate>,
    /// Số file đã hỏi được nguồn.
    pub checked: usize,
    /// File không nhận ra trên nguồn nào (mod tự build, nguồn khác).
    pub unknown: usize,
    /// File thuộc modpack, cập nhật cùng modpack.
    pub from_modpack: usize,
}

/// Một mục người dùng chọn cập nhật.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateItem {
    pub file: String,
    pub source: CatalogSource,
    pub project_id: String,
    pub version_id: String,
}

impl App {
    /// Job kiểm tra cập nhật cho mọi mod/plugin của server. Không đổi gì trên đĩa.
    pub fn check_updates(self: &Arc<Self>, id: ServerId) -> Result<JobId, AppError> {
        let spec = self.store.load(id)?;
        let _ = super::content_kind_for(&spec)?;
        let app = Arc::clone(self);
        Ok(self
            .jobs
            .spawn(JobKind::CheckUpdates, Some(id), move |ctx| async move {
                ctx.step(JobStep::ResolveContent, 1, 1);
                let report = ctx.cancellable(app.find_updates(id)).await?;
                let value = serde_json::to_value(&report).map_err(|err| {
                    AppError::new(ErrorCode::Internal, format!("kết quả kiểm tra: {err}"))
                })?;
                ctx.set_result(value);
                Ok(())
            }))
    }

    /// Job cập nhật các mục đã chọn: tắt server, sao lưu `before_update`, cài từng bản mới.
    pub fn update_content(
        self: &Arc<Self>,
        id: ServerId,
        items: Vec<UpdateItem>,
    ) -> Result<JobId, AppError> {
        if items.is_empty() || items.len() > MAX_UPDATE_ITEMS {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "danh sách cập nhật rỗng hoặc quá dài",
            )
            .with_params(serde_json::json!({ "field": "items" })));
        }
        let view = self.get(id)?;
        if view.installing.is_some() {
            return Err(AppError::new(ErrorCode::ServerBusy, "server đang được cài"));
        }
        let spec = view
            .spec
            .ok_or_else(|| AppError::new(ErrorCode::ConfigInvalid, "server hỏng cấu hình"))?;
        let _ = super::content_kind_for(&spec)?;
        for item in &items {
            let _ = self.catalog_for(item.source)?;
        }
        let world = self.claim_world(id, JobKind::UpdateContent)?;
        let app = Arc::clone(self);
        Ok(self
            .jobs
            .spawn(JobKind::UpdateContent, Some(id), move |ctx| async move {
                let _world = world;
                app.run_update(&ctx, id, &items).await
            }))
    }

    async fn run_update(
        &self,
        ctx: &JobContext,
        id: ServerId,
        items: &[UpdateItem],
    ) -> Result<(), AppError> {
        let steps = u8::try_from(items.len().saturating_add(2)).unwrap_or(u8::MAX);
        let (backup, _) = self.prepare_update(ctx, id, 1, steps).await?;
        let result = async {
            self.adopt_manual(id, items).await?;
            for item in items {
                ctx.check_cancel()?;
                let request = super::ContentRequest {
                    source: item.source,
                    project_id: item.project_id.clone(),
                    version_id: Some(item.version_id.clone()),
                };
                self.install_now(ctx, id, &[request]).await?;
            }
            Ok(())
        }
        .await;
        result.map_err(|err| with_backup(err, &backup))
    }

    async fn find_updates(&self, id: ServerId) -> Result<UpdateReport, AppError> {
        let spec = self.store.load(id)?;
        let kind = super::content_kind_for(&spec)?;
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
        let mut report = UpdateReport::default();
        let mut targets = Vec::new();
        let mut by_hash = Vec::new();
        for file in files {
            match file.record.as_ref() {
                Some(rec) if rec.from_modpack => report.from_modpack += 1,
                Some(rec) if rec.source == ContentSource::Curseforge => targets.push(Target {
                    source: CatalogSource::Curseforge,
                    project_id: rec.project_id.as_ref().map(ToString::to_string),
                    current: rec.version_id.clone(),
                    file,
                }),
                _ => by_hash.push(file),
            }
        }
        let modrinth = self.catalog_for(CatalogSource::Modrinth)?;
        let hashes: Vec<String> = by_hash
            .iter()
            .map(|f| f.sha1.to_ascii_lowercase())
            .collect();
        let known = modrinth.versions_by_hash(&hashes).await?;
        for file in by_hash {
            targets.push(modrinth_target(file, &known));
        }
        for target in targets {
            let Some(project_id) = target.project_id.clone() else {
                report.unknown += 1;
                continue;
            };
            let Ok(catalog) = self.catalog_for(target.source) else {
                // CurseForge không có key: không hỏi được, coi như chưa biết.
                report.unknown += 1;
                continue;
            };
            report.checked += 1;
            let latest = latest_version(catalog.as_ref(), &project_id, &spec).await?;
            if let Some(latest) = latest.filter(|v| Some(&v.id) != target.current.as_ref()) {
                report.updates.push(UpdateCandidate {
                    file: target.file.path.clone(),
                    source: target.source,
                    project_id,
                    current_version_id: target.current,
                    latest_version_name: latest.version_number.clone().unwrap_or(latest.name),
                    latest_version_id: latest.id,
                    manual: target.file.record.is_none(),
                });
            }
        }
        Ok(report)
    }

    /// Mod "Thêm tay" chưa có bản ghi: ghi bản ghi Modrinth để bước cài thay đúng file cũ.
    async fn adopt_manual(&self, id: ServerId, items: &[UpdateItem]) -> Result<(), AppError> {
        let spec = self.store.load(id)?;
        let kind = super::content_kind_for(&spec)?;
        let mutex = self.content_lock(id);
        let _guard = mutex.lock().await;
        let (store, scanner) = (self.store.clone(), Arc::clone(&self.content_scanner));
        let items = items.to_vec();
        blocking(move || {
            let paths = store.paths(id);
            let files = scanner.scan_dirs(&paths.server_dir, &paths.content_file, kind)?;
            let mut manifest = ContentManifest::load(&paths.content_file)?;
            let mut changed = false;
            for item in &items {
                let Some(file) = files.iter().find(|f| f.path == item.file) else {
                    return Err(AppError::not_found(&item.file));
                };
                if file.record.is_some() {
                    continue;
                }
                manifest.file.push(ContentRecord {
                    path: file.path.clone(),
                    sha1: file.sha1.clone(),
                    source: item.source.to_content_source(),
                    project_id: Some(item.source.project_id(&item.project_id)?),
                    version_id: None,
                    client_only: false,
                    from_modpack: false,
                    installed_at: OffsetDateTime::now_utc(),
                    extra: Default::default(),
                });
                changed = true;
            }
            if changed {
                manifest.save(&paths.content_file)?;
            }
            Ok(())
        })
        .await
    }
}

struct Target {
    source: CatalogSource,
    project_id: Option<String>,
    current: Option<String>,
    file: ContentFile,
}

fn modrinth_target(file: ContentFile, known: &HashMap<String, ContentVersion>) -> Target {
    let found = known.get(&file.sha1.to_ascii_lowercase());
    let recorded = file
        .record
        .as_ref()
        .filter(|rec| rec.source == ContentSource::Modrinth);
    Target {
        source: CatalogSource::Modrinth,
        project_id: found
            .map(|v| v.project_id.clone())
            .or_else(|| recorded.and_then(|r| r.project_id.as_ref().map(ToString::to_string))),
        current: found
            .map(|v| v.id.clone())
            .or_else(|| recorded.and_then(|r| r.version_id.clone())),
        file,
    }
}

/// Bản mới nhất hợp loader và Minecraft của server: phần tử đầu, cùng quy tắc với lúc cài.
async fn latest_version(
    catalog: &dyn ContentCatalog,
    project_id: &str,
    spec: &mh_core::InstanceSpec,
) -> Result<Option<ContentVersion>, AppError> {
    match catalog
        .versions(project_id, spec.game.loader, &spec.game.minecraft)
        .await
    {
        Ok(list) => Ok(list.into_iter().next()),
        // Project đã bị gỡ khỏi nguồn: không có bản mới, không làm hỏng cả lần kiểm tra.
        Err(err) if err.code == ErrorCode::ContentNotFound => Ok(None),
        Err(err) => Err(err),
    }
}
