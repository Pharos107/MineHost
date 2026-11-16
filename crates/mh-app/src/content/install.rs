//! Job cài mod/plugin: tải vào thư mục tạm rồi mới thay vào `mods/` hoặc `plugins/`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mh_core::{ContentKind, ErrorCode, ServerId};
use mh_platform::SafeRoot;
use mh_store::{
    ContentManifest, ContentRecord, ContentScanner, PendingContent, ServerStore, inspect_jar_file,
};
use time::OffsetDateTime;
use ulid::Ulid;

use super::catalog::{CatalogSource, ContentCatalog};
use super::curseforge::parse_cf_id;
use super::plan::{ContentInstallPlan, PlannedContent};
use super::requests::{ContentRequest, check_count};
use super::running::replace_while_running;
use crate::files::{blocking, delete_sync, exists};
use crate::jobs::{JobContext, JobKind, JobStep};
use crate::{App, AppError, JobId};

struct StagingGuard(PathBuf);

impl Drop for StagingGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Đếm mục đã xử lý qua mọi nguồn để thanh tiến độ chạy liền một mạch.
struct Progress {
    done: usize,
    total: u8,
}

struct StagedFile {
    project_id: String,
    version_id: String,
    file_name: String,
    staged_path: PathBuf,
    sha1: String,
    client_only: bool,
}

impl App {
    /// Tạo Job cài một mod/plugin (cùng dependency).
    pub fn install_content(
        self: &Arc<Self>,
        id: ServerId,
        source: CatalogSource,
        project_id: impl Into<String>,
        version_id: Option<String>,
    ) -> Result<JobId, AppError> {
        self.install_content_many(
            id,
            vec![ContentRequest {
                source,
                project_id: project_id.into(),
                version_id,
            }],
        )
    }

    /// Tạo một Job cài nhiều mod/plugin cùng dependency. Server đang chạy vẫn cài được mod mới;
    /// mod chỉ có hiệu lực ở lần khởi động sau.
    pub fn install_content_many(
        self: &Arc<Self>,
        id: ServerId,
        requests: Vec<ContentRequest>,
    ) -> Result<JobId, AppError> {
        check_count(requests.len())?;
        let _ = self.ensure_content_addable(id)?;
        for req in &requests {
            let _ = self.catalog_for(req.source)?;
        }
        let app = Arc::clone(self);
        Ok(self
            .jobs
            .spawn(JobKind::InstallContent, Some(id), move |ctx| async move {
                ctx.step(JobStep::CheckDisk, 1, 3);
                app.check_disk()?;
                ctx.step(JobStep::ResolveContent, 2, 3);
                app.install_now(&ctx, id, &requests).await
            }))
    }

    /// Lập kế hoạch, tải hết vào thư mục tạm rồi mới thay vào `mods/`: một mục lỗi thì không file
    /// nào được chép. Job cập nhật gọi lại hàm này cho từng mục sau khi đã sao lưu.
    pub(super) async fn install_now(
        &self,
        ctx: &JobContext,
        id: ServerId,
        requests: &[ContentRequest],
    ) -> Result<(), AppError> {
        let plans = ctx
            .cancellable(self.plan_content_requests(id, requests))
            .await?;
        let all: Vec<&PlannedContent> = plans.iter().flat_map(|p| &p.items).collect();
        let mut names = HashSet::new();
        for item in &all {
            if !names.insert(&item.file.filename) {
                return Err(exists(&item.file.filename));
            }
        }
        let staging_dir = self
            .store
            .paths(id)
            .root
            .join(format!(".content-install-{}", Ulid::generate()));
        let staging_create = staging_dir.clone();
        blocking(move || {
            fs::create_dir_all(&staging_create)
                .map_err(|e| AppError::new(ErrorCode::Internal, e.to_string()))
        })
        .await?;
        let _guard = StagingGuard(staging_dir.clone());

        let mut progress = Progress {
            done: 0,
            total: u8::try_from(all.len().saturating_add(3)).unwrap_or(u8::MAX),
        };
        let mut batches = Vec::with_capacity(plans.len());
        for plan in &plans {
            let catalog = self.catalog_for(plan.source)?;
            let (staged, pending) = self
                .download_staged(ctx, catalog.as_ref(), &staging_dir, plan, &mut progress)
                .await?;
            batches.push((plan.kind, plan.source, staged, pending));
        }
        ctx.check_cancel()?;

        ctx.step(JobStep::PrepareFiles, progress.total, progress.total);
        for (kind, source, staged, pending) in batches {
            self.commit_staged(id, kind, source, staged, pending)
                .await?;
        }
        self.refresh_preflight(id).await;
        self.notify_changed(id);
        Ok(())
    }

    async fn download_staged(
        &self,
        ctx: &JobContext,
        catalog: &dyn ContentCatalog,
        staging_dir: &Path,
        plan: &ContentInstallPlan,
        progress: &mut Progress,
    ) -> Result<(Vec<StagedFile>, Vec<PendingContent>), AppError> {
        let mut staged = Vec::new();
        let mut pending = Vec::new();
        for item in &plan.items {
            ctx.check_cancel()?;
            let idx = progress.done;
            progress.done += 1;
            if item.file.manual_download {
                pending.push(build_pending(plan.kind, item)?);
                continue;
            }
            let step = u8::try_from(idx + 3).unwrap_or(progress.total.saturating_sub(1));
            ctx.step(
                JobStep::DownloadContent {
                    name: item.project_name.clone(),
                },
                step,
                progress.total,
            );
            let staged_path = staging_dir.join(format!("{idx:04}.jar"));
            {
                let _permit = ctx.download_permit().await?;
                let mut on_bytes = |done: u64, tot: Option<u64>| ctx.bytes(done, tot);
                catalog
                    .download(&item.file, &staged_path, &mut on_bytes, &ctx.cancel)
                    .await?;
            }
            let inspect_path = staged_path.clone();
            let info = blocking(move || Ok(inspect_jar_file(&inspect_path)?)).await?;
            staged.push(StagedFile {
                project_id: item.project_id.clone(),
                version_id: item.version_id.clone(),
                file_name: item.file.filename.clone(),
                staged_path,
                sha1: info.sha1,
                client_only: item.client_only || info.meta.is_ok_and(|m| m.side.is_client_only()),
            });
        }
        Ok((staged, pending))
    }

    async fn commit_staged(
        &self,
        id: ServerId,
        kind: ContentKind,
        source: CatalogSource,
        staged: Vec<StagedFile>,
        pending: Vec<PendingContent>,
    ) -> Result<(), AppError> {
        let mutex = self.content_lock(id);
        let _lock = mutex.lock().await;
        self.ensure_content_addable(id)?;
        let running = self.running_pid(id).is_some();
        let (store, trash, scanner) = (
            self.store.clone(),
            self.dirs.trash(),
            Arc::clone(&self.content_scanner),
        );
        let added = blocking(move || {
            let ctx = ApplyCtx {
                store: &store,
                trash: &trash,
                scanner: &scanner,
                id,
                kind,
                source,
                running,
            };
            apply_staged(&ctx, staged, pending)
        })
        .await?;
        self.mark_added_while_running(id, added);
        Ok(())
    }
}

fn build_pending(kind: ContentKind, item: &PlannedContent) -> Result<PendingContent, AppError> {
    Ok(PendingContent {
        path: format!("{}/{}", kind.dir(), item.file.filename),
        name: Some(item.project_name.clone()),
        project_id: parse_cf_id(&item.project_id, "project_id")?,
        file_id: parse_cf_id(&item.version_id, "version_id")?,
        sha1: item.file.sha1.clone().unwrap_or_default(),
        page_url: item.file.download_page_url.clone().unwrap_or_default(),
        from_modpack: false,
        client_only: item.client_only,
        extra: Default::default(),
    })
}

#[derive(Clone, Copy)]
struct ApplyCtx<'a> {
    store: &'a ServerStore,
    trash: &'a Path,
    scanner: &'a ContentScanner,
    id: ServerId,
    kind: ContentKind,
    source: CatalogSource,
    /// Server đang có process: được thêm file mới, không được thay file cũ.
    running: bool,
}

/// Chép file đã tải vào chỗ, trả tên các file vừa thêm.
fn apply_staged(
    ctx: &ApplyCtx<'_>,
    staged: Vec<StagedFile>,
    pending: Vec<PendingContent>,
) -> Result<Vec<String>, AppError> {
    let ApplyCtx {
        store,
        trash,
        scanner,
        id,
        kind,
        source,
        running,
    } = *ctx;
    let paths = store.paths(id);
    fs::create_dir_all(&paths.server_dir)
        .map_err(|e| AppError::new(ErrorCode::Internal, e.to_string()))?;
    let safe_root = SafeRoot::new(&paths.server_dir)?;
    let scanned = scanner.scan_dirs(&paths.server_dir, &paths.content_file, kind)?;
    let mut manifest = ContentManifest::load(&paths.content_file)?;

    let content_source = source.to_content_source();
    let replaced_pids: HashSet<&str> = staged.iter().map(|s| s.project_id.as_str()).collect();
    let (to_trash, was_enabled) =
        collect_replaced(&scanned, &safe_root, kind, source, &replaced_pids)?;
    if running && !to_trash.is_empty() {
        return Err(replace_while_running());
    }
    let trash_set: HashSet<&Path> = to_trash.iter().map(|(_, p)| p.as_path()).collect();
    let dests = prepare_dests(&safe_root, kind, &staged, &trash_set, &was_enabled)?;

    for (rel, _) in &to_trash {
        delete_sync(store, trash, id, rel)?;
    }
    manifest
        .file
        .retain(|rec| !matches_replaced(rec, source, &replaced_pids));
    if source == CatalogSource::Curseforge {
        manifest
            .pending
            .retain(|p| !replaced_pids.contains(p.project_id.to_string().as_str()));
    }

    let now = OffsetDateTime::now_utc();
    let mut added = Vec::with_capacity(staged.len());
    for (s, dest) in staged.into_iter().zip(dests) {
        if let Some(parent) = dest.as_path().parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::new(ErrorCode::Internal, e.to_string()))?;
        }
        fs::rename(&s.staged_path, dest.as_path())
            .map_err(|e| AppError::new(ErrorCode::Internal, e.to_string()))?;
        manifest.file.push(ContentRecord {
            path: format!("{}/{}", kind.dir(), s.file_name),
            sha1: s.sha1,
            source: content_source,
            project_id: Some(source.project_id(&s.project_id)?),
            version_id: Some(s.version_id),
            client_only: s.client_only,
            from_modpack: false,
            installed_at: now,
            extra: Default::default(),
        });
        added.push(s.file_name);
    }
    if !pending.is_empty() {
        manifest.pending.retain(|old| {
            !pending
                .iter()
                .any(|new| old.project_id == new.project_id || old.path == new.path)
        });
        manifest.pending.extend(pending);
    }
    manifest.save(&paths.content_file)?;
    Ok(added)
}

fn matches_replaced(
    rec: &ContentRecord,
    source: CatalogSource,
    replaced_pids: &HashSet<&str>,
) -> bool {
    source.owns_record(rec.source, rec.project_id.as_ref())
        && rec
            .project_id
            .as_ref()
            .is_some_and(|p| replaced_pids.contains(p.to_string().as_str()))
}

/// File bản cũ sẽ vào thùng rác (đường dẫn tương đối, tuyệt đối) và project nào đang bật.
type Replaced = (Vec<(String, PathBuf)>, HashMap<String, bool>);

fn collect_replaced(
    scanned: &[mh_store::ContentFile],
    safe_root: &SafeRoot,
    kind: ContentKind,
    source: CatalogSource,
    replaced_pids: &HashSet<&str>,
) -> Result<Replaced, AppError> {
    let mut to_trash = Vec::new();
    let mut was_enabled = HashMap::new();
    for file in scanned {
        let Some(rec) = file.record.as_ref() else {
            continue;
        };
        if !matches_replaced(rec, source, replaced_pids) {
            continue;
        }
        let Some(pid) = rec.project_id.as_ref().map(ToString::to_string) else {
            continue;
        };
        let dir = if file.enabled {
            kind.dir()
        } else {
            kind.disabled_dir()
        };
        let rel = format!("{dir}/{}", file.file_name);
        let abs = safe_root.join(&rel)?.into_path_buf();
        to_trash.push((rel, abs));
        *was_enabled.entry(pid).or_insert(false) |= file.enabled;
    }
    Ok((to_trash, was_enabled))
}

fn prepare_dests(
    safe_root: &SafeRoot,
    kind: ContentKind,
    staged: &[StagedFile],
    trash_set: &HashSet<&Path>,
    was_enabled: &HashMap<String, bool>,
) -> Result<Vec<mh_platform::SafePath>, AppError> {
    let mut dests = Vec::with_capacity(staged.len());
    for s in staged {
        // Cập nhật giữ lựa chọn bật/tắt của bản cũ: không tắt lại mod người dùng đã tự bật,
        // không bật lại mod họ đã tắt (spec P3, mục 3.5). Mod mới thì theo luật chỉ chạy client.
        let enabled = was_enabled
            .get(&s.project_id)
            .copied()
            .unwrap_or(!s.client_only);
        let dir = if enabled {
            kind.dir()
        } else {
            kind.disabled_dir()
        };
        let dest = safe_root.join(&format!("{dir}/{}", s.file_name))?;
        if dest.as_path().exists() && !trash_set.contains(dest.as_path()) {
            return Err(exists(&format!("{dir}/{}", s.file_name)));
        }
        dests.push(dest);
    }
    Ok(dests)
}
