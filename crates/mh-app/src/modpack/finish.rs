//! Hoàn tất tải song song file modpack, ghi `content.toml` và `[modpack]` trong `instance.toml`.

use std::collections::HashMap;
use std::path::Path;

use mh_core::instance::{ModpackSection, UpstreamText};
use mh_core::{ContentKind, ErrorCode};
use mh_providers::modpack::{PackFile, PackPlan};
use mh_store::{ContentManifest, ContentRecord, ContentScanner, ContentSource, ProjectId};
use time::OffsetDateTime;
use tokio::task::JoinSet;

use super::ModpackMeta;
use crate::AppError;
use crate::jobs::{JobContext, JobStep};
use crate::sources::CancellationToken;

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
    pack_files: &[PackFile],
) -> Result<usize, AppError> {
    let by_path: HashMap<&str, (&str, Option<ProjectId>, Option<String>)> = pack_files
        .iter()
        .map(|f| {
            let (pid, vid) = parse_modrinth_cdn_ids(&f.urls);
            (f.path.as_str(), (f.sha1.as_str(), pid, vid))
        })
        .collect();

    let now = OffsetDateTime::now_utc();
    let now = now.replace_nanosecond(0).unwrap_or(now);
    let mut records = Vec::new();
    for kind in [ContentKind::Mods, ContentKind::Plugins] {
        for item in scanner.scan_dirs(server_dir, content_file, kind)? {
            // Chỉ gắn ID Modrinth nếu hash trên đĩa còn khớp bản đã tải (tránh gắn nhầm khi override ghi đè).
            let (project_id, version_id) = match by_path.get(item.path.as_str()) {
                Some((sha1, pid, vid)) if *sha1 == item.sha1 => (pid.clone(), vid.clone()),
                _ => (None, None),
            };
            records.push(ContentRecord {
                path: item.path,
                sha1: item.sha1,
                source: ContentSource::Modpack,
                project_id,
                version_id,
                client_only: item.meta.as_ref().is_ok_and(|m| m.side.is_client_only()),
                from_modpack: true,
                installed_at: now,
                extra: Default::default(),
            });
        }
    }
    let count = records.len();
    let manifest = ContentManifest {
        file: records,
        ..Default::default()
    };
    manifest.save(content_file)?;
    Ok(count)
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
