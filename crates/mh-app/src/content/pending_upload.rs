//! Tải tay file CurseForge cấm phân phối: khớp với mục `[[pending]]` rồi đặt vào đúng chỗ.

use std::fs;
use std::path::{Path, PathBuf};

use mh_core::{ContentKind, ErrorCode, ServerId};
use mh_platform::SafeRoot;
use mh_store::{
    ContentFile, ContentManifest, ContentRecord, ContentSource, PendingContent, ProjectId,
    ServerStore,
};
use serde_json::json;
use time::OffsetDateTime;

use super::manage::{
    PendingUploadResult, inspect_upload_source, move_file_into_place, pending_jar_name,
    persist_upload_record, validate_jar_name,
};
use crate::AppError;
use crate::files::{exists, io_err};

pub(super) fn upload_pending_batch_sync(
    store: &ServerStore,
    id: ServerId,
    kind: ContentKind,
    items: Vec<(String, PathBuf)>,
) -> Vec<PendingUploadResult> {
    items
        .into_iter()
        .map(|(file_name, source_path)| {
            let outcome = validate_jar_name(&file_name)
                .and_then(|()| upload_pending_sync(store, id, kind, &file_name, &source_path));
            PendingUploadResult { file_name, outcome }
        })
        .collect()
}

pub(super) fn upload_pending_sync(
    store: &ServerStore,
    id: ServerId,
    kind: ContentKind,
    file_name: &str,
    source_path: &Path,
) -> Result<ContentFile, AppError> {
    let (size, info) = inspect_upload_source(source_path)?;
    let paths = store.paths(id);
    let mut manifest = ContentManifest::load(&paths.content_file)?;
    let Some(idx) = find_pending(&manifest.pending, &info.sha1, file_name) else {
        return Err(hash_mismatch_error(&manifest.pending));
    };
    let matched = manifest.pending.remove(idx);
    let target_name = pending_jar_name(&matched.path)
        .ok_or_else(|| AppError::new(ErrorCode::ConfigInvalid, "đường dẫn mục chờ không hợp lệ"))?;

    // Mod chỉ chạy client (theo metadata trong jar) vào thẳng thư mục tắt (spec P3, mục 3.5).
    let client_only = info.meta.as_ref().is_ok_and(|m| m.side.is_client_only());
    let rel = if client_only && matched.path.starts_with(&format!("{}/", kind.dir())) {
        format!("{}/{target_name}", kind.disabled_dir())
    } else {
        matched.path.clone()
    };
    fs::create_dir_all(&paths.server_dir).map_err(|e| io_err(&paths.server_dir, e))?;
    let root = SafeRoot::new(&paths.server_dir)?;
    let dest = root.join(&rel)?;
    if fs::symlink_metadata(dest.as_path()).is_ok() {
        return Err(exists(&rel));
    }
    if let Some(parent) = dest.as_path().parent() {
        fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    }
    move_file_into_place(source_path, dest.as_path())?;

    let record = ContentRecord {
        path: rel.clone(),
        sha1: info.sha1.clone(),
        source: if matched.from_modpack {
            ContentSource::Modpack
        } else {
            ContentSource::Curseforge
        },
        project_id: Some(ProjectId::Int(matched.project_id)),
        version_id: Some(matched.file_id.to_string()),
        client_only,
        from_modpack: matched.from_modpack,
        installed_at: OffsetDateTime::now_utc(),
        extra: Default::default(),
    };
    persist_upload_record(&mut manifest, &paths.content_file, dest.as_path(), &record)?;
    let enabled = !rel.starts_with(&format!("{}/", kind.disabled_dir()));
    Ok(ContentFile {
        file_name: target_name,
        path: rel,
        enabled,
        size,
        sha1: info.sha1,
        formats: info.formats,
        meta: info.meta,
        format_metas: info.format_metas,
        bundled: info.bundled,
        record: Some(record),
        manual: false,
    })
}

fn hash_mismatch_error(pending: &[PendingContent]) -> AppError {
    let expected_files: Vec<String> = pending
        .iter()
        .filter_map(|p| {
            Path::new(&p.path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_owned)
        })
        .collect();
    let expected = expected_files.join(", ");
    AppError::new(
        ErrorCode::UploadHashMismatch,
        "file tải lên không khớp SHA-1 của mục chờ nào",
    )
    .with_params(json!({
        "expected": expected,
        "expected_files": expected_files,
    }))
}

/// Khớp theo SHA-1. CurseForge đôi khi không trả SHA-1 cho file cấm phân phối; mục chờ khi đó
/// có `sha1 = ""` và chỉ còn cách khớp theo tên file người dùng tải về.
fn find_pending(pending: &[PendingContent], sha1: &str, file_name: &str) -> Option<usize> {
    pending
        .iter()
        .position(|p| !p.sha1.is_empty() && p.sha1.eq_ignore_ascii_case(sha1))
        .or_else(|| {
            pending.iter().position(|p| {
                p.sha1.is_empty() && pending_jar_name(&p.path).as_deref() == Some(file_name)
            })
        })
}
