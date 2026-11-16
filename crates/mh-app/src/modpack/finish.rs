//! Hoàn tất tải song song file modpack, tắt mod client-only, ghi `content.toml` và `[modpack]`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use mh_core::instance::{GameSection, ModpackSection, UpstreamText};
use mh_core::{ContentKind, ErrorCode};
use mh_platform::SafeRoot;
use mh_providers::modpack::{BlockedPackFile, PackFile, PackPlan};
use mh_store::{
    ContentFile, ContentManifest, ContentRecord, ContentScanner, ContentSource, PendingContent,
    ProjectId, ServerPaths,
};
use time::OffsetDateTime;
use tokio::task::JoinSet;

use super::{ModpackMeta, ModpackRequest, ModpackSourceInput, ModpackSummary, PackTarget, has_ext};
use crate::content::{CatalogSource, parse_cf_id};
use crate::jobs::{JobContext, JobStep};
use crate::sources::CancellationToken;
use crate::{App, AppError};

impl App {
    pub(super) async fn finalize_modpack(
        &self,
        ctx: &JobContext,
        target: PackTarget,
        game: GameSection,
        paths: &ServerPaths,
        pack: InstalledPack,
        meta: ModpackMeta,
    ) -> Result<(), AppError> {
        let InstalledPack { plan, client_sha1 } = pack;
        let pending_curseforge: Vec<String> = plan.blocked.iter().map(|b| b.path.clone()).collect();
        let installed_mods = self
            .record_modpack_content(paths, &plan, client_sha1)
            .await?;
        let modpack = build_modpack_section(&plan, meta)?;
        self.write_server_spec(
            &paths.server_dir,
            target.id,
            target.name,
            game,
            target.memory,
            target.port,
            Some(modpack),
        )?;
        self.refresh_preflight(target.id).await;
        let summary = ModpackSummary {
            installed_mods,
            skipped_client: plan.skipped_client,
            pending_curseforge,
        };
        if let Ok(value) = serde_json::to_value(&summary) {
            ctx.set_result(value);
        }
        Ok(())
    }

    async fn record_modpack_content(
        &self,
        paths: &ServerPaths,
        plan: &PackPlan,
        client_sha1: HashSet<String>,
    ) -> Result<usize, AppError> {
        let server_dir = paths.server_dir.clone();
        let content_file = paths.content_file.clone();
        let files = plan.files.clone();
        let blocked = plan.blocked.clone();
        let scanner = Arc::clone(&self.content_scanner);
        crate::files::blocking(move || {
            let pack = PackOutput {
                files: &files,
                blocked: &blocked,
                client_sha1: &client_sha1,
            };
            write_content_manifest(&scanner, &server_dir, &content_file, &pack)
        })
        .await
    }
}

/// Pack đã tải xong, kèm SHA-1 (chữ thường) của mod mà nguồn ngoài jar cho biết chỉ chạy ở client
/// (spec P3, mục 3.5, nguồn 4). Pack không tra nguồn ngoài thì tập này rỗng.
pub(super) struct InstalledPack {
    pub plan: PackPlan,
    pub client_sha1: HashSet<String>,
}

impl From<PackPlan> for InstalledPack {
    fn from(plan: PackPlan) -> Self {
        Self {
            plan,
            client_sha1: HashSet::new(),
        }
    }
}

pub(super) async fn drain_downloads(
    ctx: &JobContext,
    set: &mut JoinSet<Result<(String, u64), AppError>>,
    child_cancel: &CancellationToken,
    total_files: usize,
    total_bytes: u64,
    step_idx: u8,
    step_count: u8,
) -> Result<(), AppError> {
    let mut done_files = 0usize;
    let mut done_bytes = 0u64;
    loop {
        let next = tokio::select! {
            () = ctx.cancel.cancelled() => {
                child_cancel.cancel();
                while set.join_next().await.is_some() {}
                return Err(AppError::cancelled());
            }
            res = set.join_next() => res,
        };
        let Some(joined) = next else {
            return ctx.check_cancel();
        };
        match joined {
            Ok(Ok((path, size))) => {
                done_files += 1;
                done_bytes = done_bytes.saturating_add(size);
                ctx.step(
                    JobStep::DownloadPackFile {
                        name: file_label(&path),
                        index: done_files,
                        total: total_files,
                    },
                    step_idx,
                    step_count,
                );
                ctx.bytes(done_bytes, Some(total_bytes));
            }
            Ok(Err(err)) => {
                child_cancel.cancel();
                while set.join_next().await.is_some() {}
                return Err(err);
            }
            Err(err) => {
                child_cancel.cancel();
                while set.join_next().await.is_some() {}
                return Err(AppError::new(
                    ErrorCode::Internal,
                    format!("lỗi tác vụ tải file modpack: {err}"),
                ));
            }
        }
    }
}

pub(super) fn write_content_manifest(
    scanner: &ContentScanner,
    server_dir: &Path,
    content_file: &Path,
    pack: &PackOutput<'_>,
) -> Result<usize, AppError> {
    let keep = KeptContent::default();
    write_pack_records(scanner, server_dir, content_file, pack, &keep, Vec::new())
}

/// File của pack vừa cài: file đã tải, file chờ tải tay, và SHA-1 (chữ thường) của mod mà nguồn
/// ngoài jar cho biết chỉ chạy ở client (spec P3, mục 3.5, nguồn 4).
pub(super) struct PackOutput<'a> {
    pub files: &'a [PackFile],
    pub blocked: &'a [BlockedPackFile],
    pub client_sha1: &'a HashSet<String>,
}

impl PackOutput<'_> {
    fn is_client_only(&self, item: &ContentFile) -> bool {
        item.meta.as_ref().is_ok_and(|m| m.side.is_client_only())
            || self.client_sha1.contains(&item.sha1.to_ascii_lowercase())
    }
}

/// Mod người dùng có trước lần cập nhật modpack: giữ nguyên bản ghi, hoặc giữ là "Thêm tay".
#[derive(Debug, Default, Clone)]
pub(super) struct KeptContent {
    pub records: HashMap<String, ContentRecord>,
    pub manual: HashSet<String>,
}

impl KeptContent {
    fn owns(&self, path: &str) -> bool {
        self.records.contains_key(path) || self.manual.contains(path)
    }
}

/// Ghi `content.toml` sau khi cài pack. Mọi file trên đĩa không thuộc `keep` đều coi là của pack
/// (file tải về hoặc jar trong `overrides/`).
pub(super) fn write_pack_records(
    scanner: &ContentScanner,
    server_dir: &Path,
    content_file: &Path,
    pack: &PackOutput<'_>,
    keep: &KeptContent,
    user_pending: Vec<PendingContent>,
) -> Result<usize, AppError> {
    disable_client_only_mods(scanner, server_dir, content_file, keep, pack)?;

    let by_path: HashMap<&str, (&str, Option<ProjectId>, Option<String>)> = pack
        .files
        .iter()
        .map(|f| {
            let (pid, vid) = match f.cf_ids {
                Some((pid, fid)) => (Some(ProjectId::Int(pid)), Some(fid.to_string())),
                None => parse_modrinth_cdn_ids(&f.urls),
            };
            (f.path.as_str(), (f.sha1.as_str(), pid, vid))
        })
        .collect();

    let now = OffsetDateTime::now_utc();
    let now = now.replace_nanosecond(0).unwrap_or(now);
    let mut records = Vec::new();
    let mut count = 0;
    for kind in [ContentKind::Mods, ContentKind::Plugins] {
        for item in scanner.scan_dirs(server_dir, content_file, kind)? {
            if let Some(rec) = keep.records.get(&item.path) {
                records.push(rec.clone());
                continue;
            }
            if keep.manual.contains(&item.path) {
                continue;
            }
            // Chỉ gắn ID nguồn nếu hash trên đĩa còn khớp bản đã tải (tránh gắn nhầm khi override ghi đè).
            let (project_id, version_id) = match by_path.get(item.path.as_str()) {
                Some((sha1, pid, vid)) if *sha1 == item.sha1 => (pid.clone(), vid.clone()),
                _ => (None, None),
            };
            count += 1;
            let client_only = pack.is_client_only(&item);
            records.push(ContentRecord {
                path: item.path,
                sha1: item.sha1,
                source: ContentSource::Modpack,
                project_id,
                version_id,
                client_only,
                from_modpack: true,
                installed_at: now,
                extra: Default::default(),
            });
        }
    }
    let mut pending = user_pending;
    pending.extend(pack.blocked.iter().map(|b| PendingContent {
        path: b.path.clone(),
        // Manifest CurseForge chỉ có id, không có tên mod; UI dùng tên file thay thế.
        name: None,
        project_id: b.project_id,
        file_id: b.file_id,
        sha1: b.sha1.clone(),
        page_url: b.page_url.clone(),
        from_modpack: true,
        client_only: pack.client_sha1.contains(&b.sha1.to_ascii_lowercase()),
        extra: Default::default(),
    }));
    let manifest = ContentManifest {
        file: records,
        pending,
        ..Default::default()
    };
    manifest.save(content_file)?;
    Ok(count)
}

/// Chuyển mod chỉ chạy client (metadata trong `.jar`, hoặc nguồn ngoài qua `client_sha1`) sang
/// `mods-disabled/` (spec P3, mục 3.5). Mod người dùng giữ lại sau lần cập nhật pack thì không đụng tới.
fn disable_client_only_mods(
    scanner: &ContentScanner,
    server_dir: &Path,
    content_file: &Path,
    keep: &KeptContent,
    pack: &PackOutput<'_>,
) -> Result<(), AppError> {
    if !server_dir.join(ContentKind::Mods.dir()).is_dir() {
        return Ok(());
    }
    let items = scanner.scan_dirs(server_dir, content_file, ContentKind::Mods)?;
    let to_disable: Vec<_> = items
        .into_iter()
        .filter(|i| !keep.owns(&i.path))
        .filter(|i| i.enabled && pack.is_client_only(i))
        .collect();
    if to_disable.is_empty() {
        return Ok(());
    }
    let root = SafeRoot::new(server_dir)?;
    let disabled_dir = root.join(ContentKind::Mods.disabled_dir())?;
    fs::create_dir_all(disabled_dir.as_path()).map_err(|e| {
        AppError::new(
            ErrorCode::Internal,
            format!("không tạo được mods-disabled: {e}"),
        )
    })?;
    for item in to_disable {
        let src = root.join(&item.path)?;
        let dst = root.join(&format!(
            "{}/{}",
            ContentKind::Mods.disabled_dir(),
            item.file_name
        ))?;
        if !dst.as_path().exists() {
            fs::rename(src.as_path(), dst.as_path()).map_err(|e| {
                AppError::new(
                    ErrorCode::Internal,
                    format!("không chuyển được mod client-only {}: {e}", item.file_name),
                )
            })?;
        }
    }
    Ok(())
}

pub(super) fn build_modpack_section(
    plan: &PackPlan,
    meta: ModpackMeta,
) -> Result<ModpackSection, AppError> {
    let name = match sanitize_upstream(&plan.name) {
        Some(v) => v,
        None => UpstreamText::parse("Modpack")
            .map_err(|e| AppError::new(ErrorCode::Internal, e.to_string()))?,
    };
    Ok(ModpackSection {
        source: meta.source,
        project_id: meta.project_id,
        version_id: meta.version_id,
        name,
        version_name: meta
            .version_name
            .or_else(|| sanitize_upstream(&plan.version_id)),
        extra: Default::default(),
    })
}

pub(super) fn file_label(rel_path: &str) -> String {
    Path::new(rel_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(rel_path)
        .to_owned()
}

fn parse_modrinth_cdn_ids(urls: &[String]) -> (Option<ProjectId>, Option<String>) {
    for raw in urls {
        let base = raw.split(['?', '#']).next().unwrap_or(raw);
        let Some((_, after_scheme)) = base.split_once("://") else {
            continue;
        };
        let parts: Vec<&str> = after_scheme.split('/').collect();
        if parts.len() >= 6
            && parts[0].eq_ignore_ascii_case("cdn.modrinth.com")
            && parts[1] == "data"
            && parts[3] == "versions"
            && !parts[2].is_empty()
            && !parts[4].is_empty()
        {
            return (
                Some(ProjectId::Str(parts[2].to_owned())),
                Some(parts[4].to_owned()),
            );
        }
    }
    (None, None)
}

pub(super) fn sanitize_upstream(raw: &str) -> Option<UpstreamText> {
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(200)
        .collect();
    UpstreamText::parse(&cleaned).ok()
}

impl App {
    pub(super) fn validate_pack_request(&self, req: &ModpackRequest) -> Result<(), AppError> {
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
            ModpackSourceInput::Curseforge {
                project_id,
                version_id,
            } => {
                // Kiểm tra key trước để thiếu key luôn trả `CURSEFORGE_UNAVAILABLE`.
                let _ = self.catalog_for(CatalogSource::Curseforge)?;
                parse_cf_id(project_id.trim(), "project_id")?;
                if let Some(v) = version_id {
                    parse_cf_id(v.trim(), "version_id")?;
                }
                Ok(())
            }
            ModpackSourceInput::UploadedFile {
                path,
                original_name,
            } => {
                if has_ext(original_name, "mrpack") || has_ext(original_name, "zip") {
                    return Ok(());
                }
                let _ = fs::remove_file(path);
                Err(AppError::new(
                    ErrorCode::ModpackInvalid,
                    "chỉ hỗ trợ file modpack .mrpack hoặc .zip",
                ))
            }
        }
    }
}
