//! Job `create_from_modpack` (spec P3, mục 3.4, 3.5, 4.1, 4.2, 5.7):
//! lập kế hoạch từ `.mrpack` → cài Java & loader → tải song song tối đa 6 file
//! → áp `overrides/` + `server-overrides/` → ghi `content.toml` và `[modpack]` trong `instance.toml`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mh_core::instance::{ContentId, ModpackSource, UpstreamText};
use mh_core::{ErrorCode, GamePort, MemoryLimits, ServerId, ServerName};
use mh_platform::SafeRoot;
use mh_providers::modpack::{PackFile, PackLimits, PackPlan};
use mh_providers::{extract_mrpack_overrides_with, plan_mrpack_with};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

mod finish;

use crate::content::ModrinthVersion;
use crate::jobs::{JobContext, JobKind, JobStep};
use crate::{App, AppError, Created, Installing, lock};
use finish::{
    build_modpack_section, drain_downloads, file_label, sanitize_upstream, write_content_manifest,
};

const PACK_DOWNLOAD_CONCURRENCY: usize = 6;

#[derive(Debug, Clone)]
pub enum ModpackSourceInput {
    Modrinth {
        project_id: String,
        version_id: Option<String>,
    },
    /// File tải lên tạm trên đĩa; Job tự xóa sau khi hoàn tất hoặc lỗi.
    UploadedFile {
        path: PathBuf,
        original_name: String,
    },
}

#[derive(Debug, Clone)]
pub struct ModpackRequest {
    pub source: ModpackSourceInput,
    pub name: ServerName,
    pub memory_mb: Option<MemoryLimits>,
    pub port: Option<GamePort>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModpackSummary {
    pub installed_mods: usize,
    pub skipped_client: Vec<String>,
    pub pending_curseforge: Vec<String>,
}

struct ModpackMeta {
    source: ModpackSource,
    project_id: Option<ContentId>,
    version_id: Option<ContentId>,
    version_name: Option<UpstreamText>,
}

struct TempFileCleanup(Option<PathBuf>);

impl Drop for TempFileCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_file(path);
        }
    }
}

impl App {
    /// Tạo server từ modpack Modrinth hoặc file `.mrpack` tải lên; trả ngay ID server và Job.
    pub fn create_from_modpack(self: &Arc<Self>, req: ModpackRequest) -> Result<Created, AppError> {
        validate_request(&req)?;
        let port = self.resolve_new_port(req.port)?;
        let memory = req
            .memory_mb
            .unwrap_or_else(|| self.suggested_modpack_memory());
        let id = ServerId::generate();
        lock(&self.installing).insert(
            id,
            Installing {
                name: req.name.clone(),
                loader: None,
                minecraft: None,
                port,
                job_id: None,
            },
        );
        let app = Arc::clone(self);
        let job_id = self
            .jobs
            .spawn(JobKind::CreateModpack, Some(id), move |ctx| async move {
                let result = app.install_modpack(&ctx, id, &req, memory, port).await;
                app.finish_install(id, result.is_ok(), "đã cài server từ modpack");
                result
            });
        if let Some(entry) = lock(&self.installing).get_mut(&id) {
            entry.job_id = Some(job_id);
        }
        self.notify_changed(id);
        Ok(Created {
            server_id: id,
            job_id,
        })
    }

    async fn install_modpack(
        &self,
        ctx: &JobContext,
        id: ServerId,
        req: &ModpackRequest,
        memory: MemoryLimits,
        port: GamePort,
    ) -> Result<(), AppError> {
        let mut cleanup = match &req.source {
            ModpackSourceInput::UploadedFile { path, .. } => TempFileCleanup(Some(path.clone())),
            ModpackSourceInput::Modrinth { .. } => TempFileCleanup(None),
        };

        ctx.step(JobStep::CheckDisk, 1, 8);
        self.check_disk()?;

        let (mrpack_path, meta) = self
            .obtain_mrpack(ctx, id, &req.source, &mut cleanup)
            .await?;

        let hosts = self.content_catalog.pack_hosts();
        let plan_path = mrpack_path.clone();
        let plan = ctx
            .cancellable(crate::files::blocking(move || {
                plan_mrpack_with(&plan_path, hosts, PackLimits::default()).map_err(AppError::from)
            }))
            .await?;

        let planned_bytes: u64 = plan.files.iter().map(|f| f.size).sum();
        self.check_disk_for(planned_bytes)?;
        self.update_installing_runtime(id, &plan);

        let (step_count, step_packs) = if plan.loader.is_mod_loader() {
            (9, 7)
        } else {
            (8, 6)
        };
        let (paths, game) = self
            .install_pack_runtime(ctx, id, &plan, 3, step_count)
            .await?;

        let root = SafeRoot::new(&paths.server_dir)?;
        self.download_pack_files(ctx, &root, &plan.files, step_packs, step_count)
            .await?;
        self.apply_pack_overrides(
            ctx,
            &mrpack_path,
            &paths.server_dir,
            step_packs + 1,
            step_count,
        )
        .await?;

        ctx.step(JobStep::PrepareFiles, step_count, step_count);
        let installed_mods = self.record_modpack_content(&paths, &plan).await?;
        let modpack = build_modpack_section(&plan, meta)?;
        self.write_server_spec(
            &paths.server_dir,
            id,
            req.name.clone(),
            game,
            memory,
            port,
            Some(modpack),
        )?;
        let summary = ModpackSummary {
            installed_mods,
            skipped_client: plan.skipped_client,
            pending_curseforge: Vec::new(),
        };
        if let Ok(value) = serde_json::to_value(&summary) {
            ctx.set_result(value);
        }
        Ok(())
    }

    async fn install_pack_runtime(
        &self,
        ctx: &JobContext,
        id: ServerId,
        plan: &PackPlan,
        step_base: u8,
        step_count: u8,
    ) -> Result<(mh_store::ServerPaths, mh_core::instance::GameSection), AppError> {
        if plan.loader.is_mod_loader() {
            self.setup_loader_server(
                ctx,
                id,
                plan.loader,
                &plan.minecraft,
                plan.loader_version.as_ref(),
                step_base,
                step_count,
            )
            .await
        } else {
            self.setup_vanilla_server(ctx, id, plan.loader, &plan.minecraft, step_base, step_count)
                .await
        }
    }

    async fn apply_pack_overrides(
        &self,
        ctx: &JobContext,
        mrpack_path: &Path,
        server_dir: &Path,
        step_idx: u8,
        step_count: u8,
    ) -> Result<(), AppError> {
        ctx.step(JobStep::ApplyOverrides, step_idx, step_count);
        let override_pack = mrpack_path.to_path_buf();
        let override_dest = server_dir.to_path_buf();
        ctx.cancellable(crate::files::blocking(move || {
            extract_mrpack_overrides_with(&override_pack, &override_dest, PackLimits::default())
                .map_err(AppError::from)
        }))
        .await
    }

    async fn obtain_mrpack(
        &self,
        ctx: &JobContext,
        id: ServerId,
        source: &ModpackSourceInput,
        cleanup: &mut TempFileCleanup,
    ) -> Result<(PathBuf, ModpackMeta), AppError> {
        match source {
            ModpackSourceInput::UploadedFile {
                path,
                original_name,
            } => {
                ctx.step(
                    JobStep::DownloadModpack {
                        name: original_name.clone(),
                    },
                    2,
                    8,
                );
                Ok((
                    path.clone(),
                    ModpackMeta {
                        source: ModpackSource::File,
                        project_id: None,
                        version_id: None,
                        version_name: None,
                    },
                ))
            }
            ModpackSourceInput::Modrinth {
                project_id,
                version_id,
            } => {
                ctx.step(
                    JobStep::DownloadModpack {
                        name: project_id.clone(),
                    },
                    2,
                    8,
                );
                let (ver, file) = self
                    .resolve_modrinth_mrpack(ctx, project_id, version_id.as_deref())
                    .await?;
                let stage_path = self.dirs.cache().join(format!("modpack-{id}.mrpack"));
                cleanup.0 = Some(stage_path.clone());
                let _permit = ctx.download_permit().await?;
                let mut progress = |done: u64, total: Option<u64>| ctx.bytes(done, total);
                self.content_catalog
                    .download(&file, &stage_path, &mut progress, &ctx.cancel)
                    .await?;
                ctx.check_cancel()?;
                Ok((
                    stage_path,
                    ModpackMeta {
                        source: ModpackSource::Modrinth,
                        project_id: ContentId::parse(project_id).ok(),
                        version_id: ContentId::parse(&ver.id).ok(),
                        version_name: sanitize_upstream(&ver.version_number),
                    },
                ))
            }
        }
    }

    async fn resolve_modrinth_mrpack(
        &self,
        ctx: &JobContext,
        project_id: &str,
        version_id: Option<&str>,
    ) -> Result<(ModrinthVersion, mh_providers::modrinth::VersionFile), AppError> {
        let ver = match version_id {
            Some(vid) => ctx.cancellable(self.content_catalog.version(vid)).await?,
            None => {
                let list = ctx
                    .cancellable(self.content_catalog.project_versions(project_id))
                    .await?;
                list.into_iter()
                    .find(|v| pick_mrpack_file(v).is_some())
                    .ok_or_else(|| {
                        AppError::new(
                            ErrorCode::ModpackInvalid,
                            "project Modrinth không có phiên bản chứa file .mrpack",
                        )
                    })?
            }
        };
        let file = pick_mrpack_file(&ver)
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::ModpackInvalid,
                    format!("phiên bản `{}` không có file .mrpack", ver.id),
                )
            })?
            .clone();
        Ok((ver, file))
    }

    fn update_installing_runtime(&self, id: ServerId, plan: &PackPlan) {
        if let Some(info) = lock(&self.installing).get_mut(&id) {
            info.loader = Some(plan.loader);
            info.minecraft = Some(plan.minecraft.clone());
        }
        self.notify_changed(id);
    }

    async fn download_pack_files(
        &self,
        ctx: &JobContext,
        root: &SafeRoot,
        files: &[PackFile],
        step_idx: u8,
        step_count: u8,
    ) -> Result<(), AppError> {
        let targets: Vec<(PackFile, PathBuf)> = files
            .iter()
            .map(|f| Ok((f.clone(), root.join(&f.path)?.into_path_buf())))
            .collect::<Result<Vec<_>, AppError>>()?;
        if targets.is_empty() {
            return Ok(());
        }
        let total_files = targets.len();
        let total_bytes: u64 = targets.iter().map(|(f, _)| f.size).sum();
        ctx.step(
            JobStep::DownloadPackFile {
                name: file_label(&targets[0].0.path),
                index: 0,
                total: total_files,
            },
            step_idx,
            step_count,
        );
        ctx.bytes(0, Some(total_bytes));

        let sem = Arc::new(Semaphore::new(PACK_DOWNLOAD_CONCURRENCY));
        // Token con chỉ hủy phần tải còn lại khi có một file lỗi, không làm Job mang trạng thái Cancelled.
        let child_cancel = ctx.cancel.child_token();
        let mut set = JoinSet::new();
        for (file, dest) in targets {
            let sem = Arc::clone(&sem);
            let catalog = Arc::clone(&self.content_catalog);
            let cancel = child_cancel.clone();
            set.spawn(async move {
                let _permit = tokio::select! {
                    () = cancel.cancelled() => return Err(AppError::cancelled()),
                    permit = sem.acquire_owned() => permit.map_err(|_| AppError::cancelled())?,
                };
                if let Some(parent) = dest.parent() {
                    tokio::fs::create_dir_all(parent).await.map_err(|e| {
                        AppError::new(ErrorCode::Internal, format!("không tạo được thư mục: {e}"))
                    })?;
                }
                let mut sink = |_: u64, _: Option<u64>| {};
                catalog
                    .download_pack_file(&file, &dest, &mut sink, &cancel)
                    .await?;
                Ok((file.path, file.size))
            });
        }

        drain_downloads(
            ctx,
            &mut set,
            &child_cancel,
            total_files,
            total_bytes,
            step_idx,
            step_count,
        )
        .await
    }

    async fn record_modpack_content(
        &self,
        paths: &mh_store::ServerPaths,
        plan: &PackPlan,
    ) -> Result<usize, AppError> {
        let server_dir = paths.server_dir.clone();
        let content_file = paths.content_file.clone();
        let files = plan.files.clone();
        let scanner = Arc::clone(&self.content_scanner);
        crate::files::blocking(move || {
            write_content_manifest(&scanner, &server_dir, &content_file, &files)
        })
        .await
    }
}

fn validate_request(req: &ModpackRequest) -> Result<(), AppError> {
    match &req.source {
        ModpackSourceInput::Modrinth {
            project_id,
            version_id,
        } => {
            if project_id.trim().is_empty()
                || version_id.as_ref().is_some_and(|v| v.trim().is_empty())
            {
                return Err(AppError::new(
                    ErrorCode::InvalidInput,
                    "project_id hoặc version_id Modrinth không được rỗng",
                ));
            }
            Ok(())
        }
        ModpackSourceInput::UploadedFile {
            path,
            original_name,
        } => {
            let lower = original_name.to_ascii_lowercase();
            if lower.ends_with(".mrpack") {
                return Ok(());
            }
            let _ = fs::remove_file(path);
            let msg = if lower.ends_with(".zip") {
                "định dạng CurseForge .zip chưa được hỗ trợ ở phiên bản này (chỉ nhận .mrpack)"
            } else {
                "chỉ hỗ trợ file modpack .mrpack"
            };
            Err(AppError::new(ErrorCode::ModpackInvalid, msg))
        }
    }
}

fn pick_mrpack_file(ver: &ModrinthVersion) -> Option<&mh_providers::modrinth::VersionFile> {
    let is_mrpack = |name: &str| {
        Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("mrpack"))
    };
    ver.files
        .iter()
        .find(|f| f.primary && is_mrpack(&f.filename))
        .or_else(|| ver.files.iter().find(|f| is_mrpack(&f.filename)))
}
