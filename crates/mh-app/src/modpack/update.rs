//! Cập nhật modpack Modrinth lên một bản khác (spec P3, mục 3.9 và tiêu chí 4).
//!
//! File của bản pack cũ (`from_modpack`) vào thùng rác; mod người dùng tự thêm hoặc cài qua
//! MineHost thì giữ. Bản pack mới cần Minecraft hoặc loader khác thì cài lại loader trong thư mục
//! server như lúc đổi phiên bản. Đổi loại loader (Fabric sang Forge) không làm trong P3.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use mh_core::instance::{ContentId, GameSection, ModpackSource};
use mh_core::{ContentKind, ErrorCode, InstanceSpec, ServerId};
use mh_platform::SafeRoot;
use mh_providers::modpack::{PackLimits, PackPlan};
use mh_providers::plan_mrpack_with;
use mh_store::{ContentManifest, ContentScanner, ServerStore};

use super::finish::{KeptContent, PackOutput, sanitize_upstream, write_pack_records};
use super::{ModpackMeta, TempFileCleanup};
use crate::content::{CatalogSource, ContentCatalog};
use crate::files::{blocking, delete_sync};
use crate::jobs::{JobContext, JobKind, JobStep};
use crate::update::with_backup;
use crate::{App, AppError, JobId};

const STEPS: u8 = 10;

/// Kết quả Job, nằm trong `result`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ModpackUpdateResult {
    pub version_id: String,
    pub installed_mods: usize,
    /// File của bản cũ đã vào thùng rác.
    pub removed: usize,
    /// Mod người dùng thêm vẫn giữ.
    pub kept: usize,
    pub pending_curseforge: Vec<String>,
    pub backup_id: String,
}

impl App {
    /// Job cập nhật modpack. Server phải được tạo từ modpack Modrinth.
    pub fn update_modpack(
        self: &Arc<Self>,
        id: ServerId,
        version_id: &str,
    ) -> Result<JobId, AppError> {
        let version_id = version_id.trim().to_owned();
        if version_id.is_empty() {
            return Err(AppError::new(ErrorCode::InvalidInput, "thiếu bản modpack")
                .with_params(serde_json::json!({ "field": "modpack_version_id" })));
        }
        let view = self.get(id)?;
        if view.installing.is_some() {
            return Err(AppError::new(ErrorCode::ServerBusy, "server đang được cài"));
        }
        let spec = view
            .spec
            .ok_or_else(|| AppError::new(ErrorCode::ConfigInvalid, "server hỏng cấu hình"))?;
        let _ = modrinth_project(&spec)?;
        let world = self.claim_world(id, JobKind::UpdateModpack)?;
        let app = Arc::clone(self);
        Ok(self
            .jobs
            .spawn(JobKind::UpdateModpack, Some(id), move |ctx| async move {
                let _world = world;
                let result = app.run_modpack_update(&ctx, id, &version_id).await?;
                if let Ok(value) = serde_json::to_value(&result) {
                    ctx.set_result(value);
                }
                Ok(())
            }))
    }

    async fn run_modpack_update(
        &self,
        ctx: &JobContext,
        id: ServerId,
        version_id: &str,
    ) -> Result<ModpackUpdateResult, AppError> {
        let spec = self.store.load(id)?;
        let project_id = modrinth_project(&spec)?;
        let catalog = self.catalog_for(CatalogSource::Modrinth)?;
        ctx.step(JobStep::ResolveContent, 1, STEPS);
        let (ver, file) = self
            .resolve_modrinth_mrpack(ctx, catalog.as_ref(), &project_id, Some(version_id))
            .await?;
        if ver.project_id != project_id {
            return Err(AppError::new(
                ErrorCode::ContentNotFound,
                "bản này không thuộc modpack của server",
            ));
        }
        let (backup, _) = self.prepare_update(ctx, id, 2, STEPS).await?;
        let meta = ModpackMeta {
            source: ModpackSource::Modrinth,
            project_id: ContentId::parse(&project_id).ok(),
            version_id: ContentId::parse(&ver.id).ok(),
            version_name: ver.version_number.as_deref().and_then(sanitize_upstream),
        };
        let stage = self
            .dirs
            .cache()
            .join(format!("modpack-update-{id}.mrpack"));
        let _cleanup = TempFileCleanup(Some(stage.clone()));
        let result = async {
            ctx.step(
                JobStep::DownloadModpack {
                    name: ver.name.clone(),
                },
                4,
                STEPS,
            );
            {
                let _permit = ctx.download_permit().await?;
                let mut progress = |done: u64, total: Option<u64>| ctx.bytes(done, total);
                catalog
                    .download(&file, &stage, &mut progress, &ctx.cancel)
                    .await?;
            }
            self.apply_pack_update(ctx, id, &catalog, &stage, meta)
                .await
        }
        .await;
        let mut done = result.map_err(|err| with_backup(err, &backup))?;
        done.backup_id = backup.id;
        Ok(done)
    }

    async fn apply_pack_update(
        &self,
        ctx: &JobContext,
        id: ServerId,
        catalog: &Arc<dyn ContentCatalog>,
        stage: &Path,
        meta: ModpackMeta,
    ) -> Result<ModpackUpdateResult, AppError> {
        let hosts = catalog.pack_hosts();
        let plan_path = stage.to_path_buf();
        let mut plan = ctx
            .cancellable(blocking(move || {
                plan_mrpack_with(&plan_path, hosts, PackLimits::default()).map_err(AppError::from)
            }))
            .await?;
        let spec = self.store.load(id)?;
        if plan.loader != spec.game.loader {
            return Err(AppError::new(
                ErrorCode::ContentIncompatible,
                "bản modpack mới dùng loại loader khác; P3 không đổi loại loader",
            ));
        }
        self.check_disk_for(plan.files.iter().map(|f| f.size).sum())?;
        let game = self.pack_runtime(ctx, &spec, &plan).await?;
        let (removed, kept) = self.remove_old_pack_files(id).await?;
        let paths = self.store.paths(id);
        let root = SafeRoot::new(&paths.server_dir)?;
        self.download_pack_files(ctx, catalog, &root, &plan.files, 8, STEPS)
            .await?;
        self.apply_pack_overrides(ctx, stage, &paths.server_dir, 9, STEPS)
            .await?;
        self.restore_server_libs(ctx, catalog, &paths, id, &mut plan, (8, STEPS))
            .await?;
        ctx.step(JobStep::PrepareFiles, STEPS, STEPS);
        let kept_count = kept.records.len() + kept.manual.len();
        let installed_mods = self.merge_pack_manifest(id, &plan, kept).await?;
        let mut spec = self.store.load(id)?;
        if let Some(game) = game {
            let extra = std::mem::take(&mut spec.game.extra);
            spec.game = GameSection { extra, ..game };
        }
        let section = super::finish::build_modpack_section(&plan, meta)?;
        spec.modpack = Some(section);
        self.store.save(&spec)?;
        self.refresh_preflight(id).await;
        self.notify_changed(id);
        Ok(ModpackUpdateResult {
            version_id: plan.version_id.clone(),
            installed_mods,
            removed,
            kept: kept_count,
            pending_curseforge: plan.blocked.iter().map(|b| b.path.clone()).collect(),
            backup_id: String::new(),
        })
    }

    /// Cài lại loader hoặc jar khi bản pack mới cần phiên bản khác. Trả `None` khi giữ nguyên.
    async fn pack_runtime(
        &self,
        ctx: &JobContext,
        spec: &InstanceSpec,
        plan: &PackPlan,
    ) -> Result<Option<GameSection>, AppError> {
        let same_mc = plan.minecraft == spec.game.minecraft;
        let same_loader = plan.loader_version.is_none()
            || plan.loader_version.as_ref() == spec.game.loader_version.as_ref();
        if same_mc && same_loader {
            return Ok(None);
        }
        let paths = self.store.paths(spec.id);
        let loader = spec.game.loader;
        let game = if loader.is_mod_loader() {
            let wanted = plan.loader_version.as_ref();
            let install = ctx
                .cancellable(self.catalog.plan(loader, &plan.minecraft, wanted))
                .await?;
            self.fetch_loader(ctx, &paths, loader, &install, 5, STEPS)
                .await?
        } else {
            let resolved = ctx
                .cancellable(self.catalog.resolve(loader, &plan.minecraft))
                .await?;
            self.fetch_vanilla(ctx, &paths, loader, resolved, 5, STEPS)
                .await?
        };
        Ok(Some(game))
    }

    /// Chuyển file của bản pack cũ vào thùng rác. Trả số file đã bỏ và mod người dùng giữ lại.
    async fn remove_old_pack_files(&self, id: ServerId) -> Result<(usize, KeptContent), AppError> {
        let (store, trash, scanner) = (
            self.store.clone(),
            self.dirs.trash(),
            Arc::clone(&self.content_scanner),
        );
        let mutex = self.content_lock(id);
        let _guard = mutex.lock().await;
        blocking(move || remove_pack_files(&store, &trash, &scanner, id)).await
    }

    async fn merge_pack_manifest(
        &self,
        id: ServerId,
        plan: &PackPlan,
        kept: KeptContent,
    ) -> Result<usize, AppError> {
        let paths = self.store.paths(id);
        let (files, blocked) = (plan.files.clone(), plan.blocked.clone());
        let scanner = Arc::clone(&self.content_scanner);
        let mutex = self.content_lock(id);
        let _guard = mutex.lock().await;
        blocking(move || {
            let old = ContentManifest::load(&paths.content_file)?;
            // Mục chờ tải tay của pack cũ không có tên (`name = None`); mục người dùng cài thì có.
            let user_pending = old.pending.into_iter().filter(|p| p.name.is_some());
            // Chỉ cập nhật được pack Modrinth: mod chỉ client đã lọc theo `.mrpack` và metadata jar.
            let none = std::collections::HashSet::new();
            let pack = PackOutput {
                files: &files,
                blocked: &blocked,
                client_sha1: &none,
            };
            write_pack_records(
                &scanner,
                &paths.server_dir,
                &paths.content_file,
                &pack,
                &kept,
                user_pending.collect(),
            )
        })
        .await
    }
}

fn modrinth_project(spec: &InstanceSpec) -> Result<String, AppError> {
    match &spec.modpack {
        Some(pack) if pack.source == ModpackSource::Modrinth => pack
            .project_id
            .as_ref()
            .map(|p| p.as_str().to_owned())
            .ok_or_else(|| not_updatable("modpack thiếu project id")),
        Some(_) => Err(not_updatable(
            "chỉ cập nhật được modpack Modrinth; modpack CurseForge hoặc file tải lên chưa hỗ trợ",
        )),
        None => Err(not_updatable("server không được tạo từ modpack")),
    }
}

fn not_updatable(detail: &str) -> AppError {
    AppError::new(ErrorCode::ContentIncompatible, detail)
}

fn remove_pack_files(
    store: &ServerStore,
    trash: &Path,
    scanner: &ContentScanner,
    id: ServerId,
) -> Result<(usize, KeptContent), AppError> {
    let paths = store.paths(id);
    let manifest = ContentManifest::load(&paths.content_file)?;
    let from_pack: HashSet<&str> = manifest
        .file
        .iter()
        .filter(|r| r.from_modpack)
        .map(|r| r.path.as_str())
        .collect();
    let mut removed = 0;
    let mut kept = KeptContent::default();
    for kind in [ContentKind::Mods, ContentKind::Plugins] {
        for file in scanner.scan_dirs(&paths.server_dir, &paths.content_file, kind)? {
            if !from_pack.contains(file.path.as_str()) {
                match file.record {
                    Some(rec) => {
                        kept.records.insert(rec.path.clone(), rec);
                    }
                    None => {
                        kept.manual.insert(file.path);
                    }
                }
                continue;
            }
            let dir = if file.enabled {
                kind.dir()
            } else {
                kind.disabled_dir()
            };
            delete_sync(store, trash, id, &format!("{dir}/{}", file.file_name))?;
            removed += 1;
        }
    }
    Ok((removed, kept))
}
