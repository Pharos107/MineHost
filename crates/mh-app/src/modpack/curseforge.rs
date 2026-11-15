//! Nhánh cài modpack CurseForge (qua API hoặc file `.zip` tải lên, spec P3 mục 3.5, 3.6, 5.7).

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use mh_core::instance::{ContentId, ModpackSource};
use mh_core::{CfManifest, CfRuntime, ErrorCode};
use mh_platform::SafeRoot;
use mh_providers::modpack::PackPlan;
use mh_providers::{
    extract_curseforge_overrides_after, extract_curseforge_server_pack_with,
    inspect_curseforge_pack_with, server_pack_has_mods,
};

use super::finish::{InstalledPack, sanitize_upstream};
use super::{ModpackMeta, PackTarget, TempFileCleanup};
use crate::content::{CatalogSource, ContentCatalog, ContentFileInfo, ContentVersion};
use crate::files::blocking;
use crate::jobs::{JobContext, JobStep};
use crate::{App, AppError};

struct CfPackSpec {
    manifest: CfManifest,
    runtime: CfRuntime,
    meta: ModpackMeta,
}

impl App {
    pub(super) async fn install_curseforge_api(
        &self,
        ctx: &JobContext,
        target: PackTarget,
        project_id: &str,
        version_id: Option<&str>,
    ) -> Result<(), AppError> {
        let catalog = self.catalog_for(CatalogSource::Curseforge)?;
        ctx.step(
            JobStep::DownloadModpack {
                name: project_id.to_owned(),
            },
            2,
            9,
        );
        let (ver, file) =
            resolve_cf_pack_version(ctx, catalog.as_ref(), project_id, version_id).await?;
        let stage_pack = self.dirs.cache().join(format!("modpack-{}.zip", target.id));
        let _cleanup = TempFileCleanup(Some(stage_pack.clone()));
        {
            let _permit = ctx.download_permit().await?;
            let mut progress = |done: u64, total: Option<u64>| ctx.bytes(done, total);
            catalog
                .download(&file, &stage_pack, &mut progress, &ctx.cancel)
                .await?;
        }
        ctx.check_cancel()?;

        let (manifest, runtime) = inspect_pack_blocking(ctx, &stage_pack).await?;
        let meta = ModpackMeta {
            source: ModpackSource::Curseforge,
            project_id: ContentId::parse(project_id).ok(),
            version_id: ContentId::parse(&ver.id).ok(),
            version_name: sanitize_upstream(&manifest.version)
                .or_else(|| sanitize_upstream(&ver.name)),
        };
        let spec = CfPackSpec {
            manifest,
            runtime,
            meta,
        };

        if let Some(sp_file) = ctx
            .cancellable(catalog.curseforge_server_pack(&ver.id))
            .await?
        {
            if !sp_file.manual_download && sp_file.url.is_some() {
                let stage_sp = self
                    .dirs
                    .cache()
                    .join(format!("modpack-{}-server.zip", target.id));
                let _cleanup_sp = TempFileCleanup(Some(stage_sp.clone()));
                if self
                    .download_server_pack(ctx, catalog.as_ref(), &sp_file, &stage_sp)
                    .await?
                {
                    return self
                        .install_cf_server_pack(ctx, target, &stage_sp, spec)
                        .await;
                }
                tracing::info!(
                    project_id,
                    version_id = %ver.id,
                    "server pack CurseForge không có mod (kiểu tải mod lúc chạy), dựng từ manifest"
                );
            } else {
                tracing::info!(
                    project_id,
                    version_id = %ver.id,
                    "server pack CurseForge cấm phân phối hoặc thiếu downloadUrl, dựng từ manifest"
                );
            }
        }

        self.install_cf_from_manifest(ctx, target, Some(catalog), &stage_pack, spec)
            .await
    }

    pub(super) async fn install_curseforge_zip(
        &self,
        ctx: &JobContext,
        target: PackTarget,
        zip_path: &Path,
        original_name: &str,
    ) -> Result<(), AppError> {
        ctx.step(
            JobStep::DownloadModpack {
                name: original_name.to_owned(),
            },
            2,
            9,
        );
        let (manifest, runtime) = inspect_pack_blocking(ctx, zip_path).await?;
        let catalog = if manifest.files.is_empty() {
            None
        } else {
            Some(self.catalog_for(CatalogSource::Curseforge)?)
        };
        let meta = ModpackMeta {
            source: ModpackSource::File,
            project_id: None,
            version_id: None,
            version_name: sanitize_upstream(&manifest.version),
        };
        let spec = CfPackSpec {
            manifest,
            runtime,
            meta,
        };
        self.install_cf_from_manifest(ctx, target, catalog, zip_path, spec)
            .await
    }

    /// Tải server pack về `stage_sp`. Trả `false` khi pack không có jar nào trong `mods/`.
    async fn download_server_pack(
        &self,
        ctx: &JobContext,
        catalog: &dyn ContentCatalog,
        sp_file: &ContentFileInfo,
        stage_sp: &Path,
    ) -> Result<bool, AppError> {
        self.check_disk_for(sp_file.size)?;
        {
            let _permit = ctx.download_permit().await?;
            let mut progress = |done: u64, total: Option<u64>| ctx.bytes(done, total);
            catalog
                .download(sp_file, stage_sp, &mut progress, &ctx.cancel)
                .await?;
        }
        ctx.check_cancel()?;
        let path = stage_sp.to_path_buf();
        ctx.cancellable(blocking(move || {
            server_pack_has_mods(&path).map_err(AppError::from)
        }))
        .await
    }

    async fn install_cf_server_pack(
        &self,
        ctx: &JobContext,
        target: PackTarget,
        stage_sp: &Path,
        spec: CfPackSpec,
    ) -> Result<(), AppError> {
        let plan = empty_cf_plan(&spec.manifest, spec.runtime);
        self.update_installing_runtime(target.id, &plan);
        let (step_count, step_apply) = if plan.loader.is_mod_loader() {
            (8, 7)
        } else {
            (7, 6)
        };
        let (paths, game) = self
            .install_pack_runtime(ctx, target.id, &plan, 3, step_count)
            .await?;

        ctx.step(JobStep::ApplyOverrides, step_apply, step_count);
        let dest_dir = paths.server_dir.clone();
        let sp_path = stage_sp.to_path_buf();
        let skipped = ctx
            .cancellable(blocking(move || {
                extract_curseforge_server_pack_with(
                    &sp_path,
                    &dest_dir,
                    mh_providers::PackLimits::default(),
                )
                .map_err(AppError::from)
            }))
            .await?;
        tracing::info!(
            server = %target.id,
            skipped_count = skipped.len(),
            "đã giải nén server pack CurseForge và bỏ các file khởi chạy/installer"
        );

        ctx.step(JobStep::PrepareFiles, step_count, step_count);
        // Server pack do tác giả làm cho server: không cần tra mod chỉ client ở nguồn ngoài.
        self.finalize_modpack(ctx, target, game, &paths, plan.into(), spec.meta)
            .await
    }

    async fn install_cf_from_manifest(
        &self,
        ctx: &JobContext,
        target: PackTarget,
        catalog: Option<Arc<dyn ContentCatalog>>,
        pack_path: &Path,
        spec: CfPackSpec,
    ) -> Result<(), AppError> {
        let limits = mh_providers::PackLimits::default();
        // Chặn manifest quá nhiều file trước khi gọi API: giới hạn chống zip bomb áp cho cả pack.
        if spec.manifest.files.len() > limits.max_files {
            return Err(pack_too_big("số file trong manifest vượt giới hạn"));
        }
        let (files, blocked) = match catalog.as_ref() {
            Some(cat) if !spec.manifest.files.is_empty() => {
                ctx.step(JobStep::ResolveContent, 2, 9);
                ctx.cancellable(cat.resolve_curseforge_files(&spec.manifest.files))
                    .await?
            }
            _ => (Vec::new(), Vec::new()),
        };
        let planned_bytes = files
            .iter()
            .try_fold(0u64, |sum, f| sum.checked_add(f.size))
            .filter(|&sum| sum <= limits.max_total_bytes)
            .ok_or_else(|| pack_too_big("tổng kích thước mod trong manifest vượt giới hạn"))?;
        self.check_disk_for(planned_bytes)?;
        let planned_files = files.len();

        let mut plan = empty_cf_plan(&spec.manifest, spec.runtime);
        plan.files = files;
        plan.blocked = blocked;
        self.update_installing_runtime(target.id, &plan);

        let (step_count, step_packs) = if plan.loader.is_mod_loader() {
            (9, 7)
        } else {
            (8, 6)
        };
        let (paths, game) = self
            .install_pack_runtime(ctx, target.id, &plan, 3, step_count)
            .await?;

        if !plan.files.is_empty() {
            let cat = catalog.as_ref().ok_or_else(|| {
                AppError::new(
                    ErrorCode::CurseforgeUnavailable,
                    "chưa có catalog CurseForge để tải modpack",
                )
            })?;
            let root = SafeRoot::new(&paths.server_dir)?;
            self.download_pack_files(ctx, cat, &root, &plan.files, step_packs, step_count)
                .await?;
        }

        ctx.step(JobStep::ApplyOverrides, step_packs + 1, step_count);
        let override_src = pack_path.to_path_buf();
        let override_dst = paths.server_dir.clone();
        ctx.cancellable(blocking(move || {
            extract_curseforge_overrides_after(
                &override_src,
                &override_dst,
                limits,
                planned_files,
                planned_bytes,
            )
            .map_err(AppError::from)
        }))
        .await?;

        ctx.step(JobStep::PrepareFiles, step_count, step_count);
        let client_sha1 = self.cf_client_only_hashes(ctx, &plan).await?;
        let pack = InstalledPack { plan, client_sha1 };
        self.finalize_modpack(ctx, target, game, &paths, pack, spec.meta)
            .await
    }

    /// Pack dựng từ `manifest.json` là bản cho client. CurseForge không ghi mod nào chỉ chạy ở
    /// client, nên tra SHA-1 của jar trong `mods/` trên Modrinth (spec P3, mục 3.5, nguồn 4). Lỗi
    /// mạng chỉ ghi log: pack vẫn cài, mod chỉ client có metadata trong jar vẫn bị tắt như cũ.
    async fn cf_client_only_hashes(
        &self,
        ctx: &JobContext,
        plan: &PackPlan,
    ) -> Result<HashSet<String>, AppError> {
        let in_mods = |path: &str| path.starts_with("mods/") && path.ends_with(".jar");
        let sha1: Vec<String> = plan
            .files
            .iter()
            .filter(|f| in_mods(&f.path))
            .map(|f| f.sha1.to_ascii_lowercase())
            .chain(
                plan.blocked
                    .iter()
                    .filter(|b| in_mods(&b.path))
                    .map(|b| b.sha1.to_ascii_lowercase()),
            )
            .collect();
        if sha1.is_empty() {
            return Ok(HashSet::new());
        }
        let lookup = self.content_catalog.server_unsupported_by_hash(&sha1);
        match ctx.cancellable(lookup).await {
            Ok(found) => {
                if !found.is_empty() {
                    tracing::info!(count = found.len(), "tắt mod chỉ chạy client theo Modrinth");
                }
                Ok(found)
            }
            Err(err) => {
                ctx.check_cancel()?;
                tracing::warn!(%err, "không tra được mod chỉ chạy client trên Modrinth");
                Ok(HashSet::new())
            }
        }
    }
}

async fn inspect_pack_blocking(
    ctx: &JobContext,
    pack_path: &Path,
) -> Result<(CfManifest, CfRuntime), AppError> {
    let path = pack_path.to_path_buf();
    ctx.cancellable(blocking(move || {
        let manifest = inspect_curseforge_pack_with(&path, mh_providers::PackLimits::default())?;
        let runtime = manifest
            .resolve_runtime()
            .map_err(|e| AppError::new(ErrorCode::ModpackInvalid, e.to_string()))?;
        Ok((manifest, runtime))
    }))
    .await
}

async fn resolve_cf_pack_version(
    ctx: &JobContext,
    catalog: &dyn ContentCatalog,
    project_id: &str,
    version_id: Option<&str>,
) -> Result<(ContentVersion, ContentFileInfo), AppError> {
    let ver = match version_id {
        Some(vid) => ctx.cancellable(catalog.version(vid)).await?,
        None => {
            let list = ctx
                .cancellable(catalog.project_versions(project_id))
                .await?;
            list.into_iter()
                .find(|v| !v.files.is_empty())
                .ok_or_else(|| {
                    AppError::new(
                        ErrorCode::ModpackInvalid,
                        "project CurseForge không có phiên bản nào để tải",
                    )
                })?
        }
    };
    let file = ver
        .files
        .first()
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::ModpackInvalid,
                format!("phiên bản `{}` không có file .zip", ver.id),
            )
        })?
        .clone();
    Ok((ver, file))
}

fn empty_cf_plan(manifest: &CfManifest, runtime: CfRuntime) -> PackPlan {
    PackPlan {
        name: manifest.name.clone(),
        version_id: manifest.version.clone(),
        loader: runtime.loader,
        minecraft: runtime.minecraft,
        loader_version: runtime.loader_version,
        files: Vec::new(),
        skipped_client: Vec::new(),
        server_unsupported: Vec::new(),
        blocked: Vec::new(),
    }
}

fn pack_too_big(reason: &str) -> AppError {
    AppError::new(ErrorCode::ModpackInvalid, reason.to_owned())
}
