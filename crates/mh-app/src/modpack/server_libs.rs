//! Tải lại thư viện mà `.mrpack` gắn cờ sai `env.server = "unsupported"` (spec P3, mục 3.5).
//!
//! Nhiều pack đánh dấu thư viện như `athena`, `s_lib` là "chỉ client" trong khi mod chạy trên server
//! bắt buộc cần chúng, và loader từ chối khởi động. Sau khi cài xong, nếu mod chạy trên server còn
//! thiếu dependency bắt buộc, MineHost tải các jar "unsupported" của pack về thư mục tạm, đọc
//! metadata và cài những jar cung cấp đúng id còn thiếu, lặp tới khi không thêm được gì.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mh_core::{ContentKind, ErrorCode, ServerId};
use mh_platform::SafeRoot;
use mh_providers::modpack::{PackFile, PackPlan};
use mh_store::{JarInfo, ServerPaths, inspect_jar_file};

use super::finish::file_label;
use crate::content::{ContentCatalog, missing_server_deps, reads_mods_toml_deps};
use crate::jobs::JobContext;
use crate::{App, AppError};

/// Thư mục tạm trong `server/`, xóa ngay sau bước này.
const STAGING_DIR: &str = ".minehost-server-libs";
/// Mỗi vòng có thể kéo theo dependency của thư viện vừa thêm; vài vòng là đủ, giới hạn để chắc dừng.
const MAX_ROUNDS: usize = 5;

struct Candidate {
    original: PackFile,
    staged: PathBuf,
    info: JarInfo,
}

impl App {
    /// Cài lại thư viện bắt buộc bị pack gắn cờ "chỉ client". Lỗi tải hay đọc chỉ ghi log: server
    /// vẫn cài xong như trước, preflight sẽ báo phần còn thiếu.
    pub(super) async fn restore_server_libs(
        &self,
        ctx: &JobContext,
        catalog: &Arc<dyn ContentCatalog>,
        paths: &ServerPaths,
        id: ServerId,
        plan: &mut PackPlan,
        step: (u8, u8),
    ) -> Result<(), AppError> {
        if plan.server_unsupported.is_empty() || !reads_mods_toml_deps(plan.loader, &plan.minecraft)
        {
            return Ok(());
        }
        let missing = self.missing_after_install(paths, id, plan).await?;
        if missing.is_empty() {
            return Ok(());
        }
        let staging = paths.server_dir.join(STAGING_DIR);
        let result = self
            .restore_from_staging(ctx, catalog, paths, id, plan, missing, step)
            .await;
        let _ = fs::remove_dir_all(&staging);
        match result {
            Err(err) if err.code == ErrorCode::JobCancelled => Err(err),
            Err(err) => {
                tracing::warn!(server = %id, %err, "không tìm lại được thư viện bị gắn cờ chỉ client");
                Ok(())
            }
            Ok(()) => Ok(()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn restore_from_staging(
        &self,
        ctx: &JobContext,
        catalog: &Arc<dyn ContentCatalog>,
        paths: &ServerPaths,
        id: ServerId,
        plan: &mut PackPlan,
        mut missing: HashSet<String>,
        (step_idx, step_count): (u8, u8),
    ) -> Result<(), AppError> {
        let staged: Vec<PackFile> = plan
            .server_unsupported
            .iter()
            .map(|f| PackFile {
                path: format!("{STAGING_DIR}/{}", file_label(&f.path)),
                ..f.clone()
            })
            .collect();
        self.check_disk_for(staged.iter().map(|f| f.size).sum())?;
        let root = SafeRoot::new(&paths.server_dir)?;
        self.download_pack_files(ctx, catalog, &root, &staged, step_idx, step_count)
            .await?;
        let mut candidates = inspect_candidates(&root, &plan.server_unsupported, &staged).await?;
        for _ in 0..MAX_ROUNDS {
            let picked: Vec<Candidate> = take_matching(&mut candidates, &missing);
            if picked.is_empty() {
                break;
            }
            for cand in picked {
                install_candidate(&root, plan, cand)?;
            }
            missing = self.missing_after_install(paths, id, plan).await?;
            if missing.is_empty() {
                break;
            }
        }
        Ok(())
    }

    async fn missing_after_install(
        &self,
        paths: &ServerPaths,
        id: ServerId,
        plan: &PackPlan,
    ) -> Result<HashSet<String>, AppError> {
        let scanner = Arc::clone(&self.content_scanner);
        let server_dir = paths.server_dir.clone();
        let loader = plan.loader;
        crate::files::blocking(move || {
            // File manifest không tồn tại: chỉ quét đĩa, không đụng `content.toml` thật.
            let scratch = server_dir.join(STAGING_DIR).join("scan.toml");
            let files = scanner.scan_dirs(&server_dir, &scratch, ContentKind::Mods)?;
            Ok(missing_server_deps(loader, &server_dir, id, &files))
        })
        .await
    }
}

async fn inspect_candidates(
    root: &SafeRoot,
    originals: &[PackFile],
    staged: &[PackFile],
) -> Result<Vec<Candidate>, AppError> {
    let pairs: Vec<(PackFile, PathBuf)> = originals
        .iter()
        .zip(staged)
        .map(|(orig, st)| Ok((orig.clone(), root.join(&st.path)?.into_path_buf())))
        .collect::<Result<_, AppError>>()?;
    crate::files::blocking(move || {
        Ok(pairs
            .into_iter()
            .filter_map(|(original, staged)| {
                let info = inspect_jar_file(&staged).ok()?;
                Some(Candidate {
                    original,
                    staged,
                    info,
                })
            })
            .collect())
    })
    .await
}

/// Lấy ra các jar chạy được trên server và cung cấp ít nhất một id đang thiếu.
fn take_matching(candidates: &mut Vec<Candidate>, missing: &HashSet<String>) -> Vec<Candidate> {
    let (picked, rest): (Vec<_>, Vec<_>) = std::mem::take(candidates).into_iter().partition(|c| {
        !is_client_only(&c.info) && provided_ids(&c.info).any(|i| missing.contains(&i))
    });
    *candidates = rest;
    picked
}

fn is_client_only(info: &JarInfo) -> bool {
    info.meta.as_ref().is_ok_and(|m| m.side.is_client_only())
}

fn provided_ids(info: &JarInfo) -> impl Iterator<Item = String> + '_ {
    info.meta
        .iter()
        .chain(info.format_metas.iter().map(|(_, m)| m))
        .chain(info.bundled.iter())
        .flat_map(|m| std::iter::once(&m.id).chain(&m.provides))
        .map(|id| id.to_ascii_lowercase())
}

fn install_candidate(
    root: &SafeRoot,
    plan: &mut PackPlan,
    cand: Candidate,
) -> Result<(), AppError> {
    let dest = root.join(&cand.original.path)?;
    if dest.as_path().exists() {
        return Ok(());
    }
    move_file(&cand.staged, dest.as_path())?;
    tracing::info!(
        path = cand.original.path,
        "cài lại thư viện bị pack gắn cờ chỉ client"
    );
    plan.skipped_client.retain(|p| p != &cand.original.path);
    plan.server_unsupported
        .retain(|f| f.path != cand.original.path);
    plan.files.push(cand.original);
    Ok(())
}

fn move_file(src: &Path, dst: &Path) -> Result<(), AppError> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            AppError::new(ErrorCode::Internal, format!("không tạo được thư mục: {e}"))
        })?;
    }
    fs::rename(src, dst)
        .map_err(|e| AppError::new(ErrorCode::Internal, format!("không chuyển được file: {e}")))
}
