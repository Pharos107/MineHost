//! Job cài mod/plugin: tải vào thư mục tạm rồi mới thay vào `mods/` hoặc `plugins/`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mh_core::{ContentKind, ErrorCode, ServerId};
use mh_platform::SafeRoot;
use mh_store::{
    ContentManifest, ContentRecord, ContentScanner, ContentSource, ProjectId, ServerStore,
    inspect_jar_file,
};
use time::OffsetDateTime;
use ulid::Ulid;

use super::plan::PlannedContent;
use crate::files::{blocking, delete_sync, exists};
use crate::jobs::{JobContext, JobKind, JobStep};
use crate::{App, AppError, JobId};

struct StagingGuard(PathBuf);

impl Drop for StagingGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
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
    /// Tạo Job cài mod/plugin (cùng dependency) cho server đang tắt.
    pub fn install_content(
        self: &Arc<Self>,
        id: ServerId,
        project_id: impl Into<String>,
        version_id: Option<String>,
    ) -> Result<JobId, AppError> {
        let _ = self.ensure_content_idle(id)?;
        let project_id = project_id.into();
        let app = Arc::clone(self);
        Ok(self
            .jobs
            .spawn(JobKind::InstallContent, Some(id), move |ctx| async move {
                app.run_install(&ctx, id, &project_id, version_id.as_deref())
                    .await
            }))
    }

    async fn run_install(
        &self,
        ctx: &JobContext,
        id: ServerId,
        project_id: &str,
        version_id: Option<&str>,
    ) -> Result<(), AppError> {
        ctx.step(JobStep::CheckDisk, 1, 3);
        self.check_disk()?;

        ctx.step(JobStep::ResolveContent, 2, 3);
        let plan = ctx
            .cancellable(self.plan_content_install(id, project_id, version_id))
            .await?;
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

        let staged = self.download_staged(ctx, &staging_dir, &plan.items).await?;
        ctx.check_cancel()?;

        let total = u8::try_from(plan.items.len().saturating_add(3)).unwrap_or(u8::MAX);
        ctx.step(JobStep::PrepareFiles, total, total);
        self.commit_staged(id, plan.kind, staged).await?;
        self.notify_changed(id);
        Ok(())
    }

    async fn download_staged(
        &self,
        ctx: &JobContext,
        staging_dir: &Path,
        items: &[PlannedContent],
    ) -> Result<Vec<StagedFile>, AppError> {
        let mut names = HashSet::new();
        for item in items {
            if !names.insert(&item.file.filename) {
                return Err(exists(&item.file.filename));
            }
        }

        let total = u8::try_from(items.len().saturating_add(3)).unwrap_or(u8::MAX);
        let mut staged = Vec::with_capacity(items.len());
        for (idx, item) in items.iter().enumerate() {
            ctx.check_cancel()?;
            let step = u8::try_from(idx + 3).unwrap_or(total.saturating_sub(1));
            ctx.step(
                JobStep::DownloadContent {
                    name: item.project_name.clone(),
                },
                step,
                total,
            );
            let staged_path = staging_dir.join(format!("{idx:04}.jar"));
            {
                let _permit = ctx.download_permit().await?;
                let mut progress = |done: u64, tot: Option<u64>| ctx.bytes(done, tot);
                self.content_catalog
                    .download(&item.file, &staged_path, &mut progress, &ctx.cancel)
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
        Ok(staged)
    }

    async fn commit_staged(
        &self,
        id: ServerId,
        kind: ContentKind,
        staged: Vec<StagedFile>,
    ) -> Result<(), AppError> {
        let mutex = self.content_lock(id);
        let _lock = mutex.lock().await;
        self.ensure_content_idle(id)?;
        let (store, trash, scanner) = (
            self.store.clone(),
            self.dirs.trash(),
            Arc::clone(&self.content_scanner),
        );
        blocking(move || apply_staged(&store, &trash, &scanner, id, kind, staged)).await
    }
}

fn apply_staged(
    store: &ServerStore,
    trash: &Path,
    scanner: &ContentScanner,
    id: ServerId,
    kind: ContentKind,
    staged: Vec<StagedFile>,
) -> Result<(), AppError> {
    let paths = store.paths(id);
    fs::create_dir_all(&paths.server_dir)
        .map_err(|e| AppError::new(ErrorCode::Internal, e.to_string()))?;
    let safe_root = SafeRoot::new(&paths.server_dir)?;
    let scanned = scanner.scan_dirs(&paths.server_dir, &paths.content_file, kind)?;
    let mut manifest = ContentManifest::load(&paths.content_file)?;

    let replaced_pids: HashSet<&str> = staged.iter().map(|s| s.project_id.as_str()).collect();
    let (to_trash, was_enabled) = collect_replaced(&scanned, &safe_root, kind, &replaced_pids)?;
    let trash_set: HashSet<&Path> = to_trash.iter().map(|(_, p)| p.as_path()).collect();
    let dests = prepare_dests(&safe_root, kind, &staged, &trash_set, &was_enabled)?;

    for (rel, _) in &to_trash {
        delete_sync(store, trash, id, rel)?;
    }
    manifest.file.retain(|rec| {
        !matches!(&rec.project_id, Some(ProjectId::Str(pid)) if replaced_pids.contains(pid.as_str()))
    });

    let now = OffsetDateTime::now_utc();
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
            source: ContentSource::Modrinth,
            project_id: Some(ProjectId::Str(s.project_id)),
            version_id: Some(s.version_id),
            client_only: s.client_only,
            from_modpack: false,
            installed_at: now,
            extra: Default::default(),
        });
    }
    manifest.save(&paths.content_file)?;
    Ok(())
}

/// File bản cũ sẽ vào thùng rác (đường dẫn tương đối, tuyệt đối) và project nào đang bật.
type Replaced = (Vec<(String, PathBuf)>, HashMap<String, bool>);

fn collect_replaced(
    scanned: &[mh_store::ContentFile],
    safe_root: &SafeRoot,
    kind: ContentKind,
    replaced_pids: &HashSet<&str>,
) -> Result<Replaced, AppError> {
    let mut to_trash = Vec::new();
    let mut was_enabled = HashMap::new();
    for file in scanned {
        let Some(ProjectId::Str(pid)) = file.record.as_ref().and_then(|r| r.project_id.as_ref())
        else {
            continue;
        };
        if replaced_pids.contains(pid.as_str()) {
            let dir = if file.enabled {
                kind.dir()
            } else {
                kind.disabled_dir()
            };
            let rel = format!("{dir}/{}", file.file_name);
            let abs = safe_root.join(&rel)?.into_path_buf();
            to_trash.push((rel, abs));
            *was_enabled.entry(pid.clone()).or_insert(false) |= file.enabled;
        }
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
