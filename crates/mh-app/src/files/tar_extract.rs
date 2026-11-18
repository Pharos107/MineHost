//! Giải nén an toàn `.tar`, `.tar.gz`, `.tgz` cho trang Tệp (spec nâng cấp trang Tệp).

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use mh_core::ErrorCode;
use mh_platform::SafeRoot;
use tar::{Archive, EntryType};

use super::archive::{CHUNK, DISK_MARGIN, EXTRACT_MAX_ENTRIES, make_dirs_inside, slash_path};
use super::disk::{Node, join_user, joined_file, node_at};
use super::{AppError, child_path, exists, invalid, io_err, outside};

pub(crate) type Progress<'a> = &'a mut dyn FnMut(u64) -> Result<(), AppError>;

#[derive(Debug)]
pub(crate) struct TarExtractPlan {
    source: PathBuf,
    gzip: bool,
    root: PathBuf,
    dest: PathBuf,
    pub(crate) total: u64,
}

pub(crate) fn is_tar_archive(path: &str) -> Option<bool> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        Some(true)
    } else if lower.ends_with(".tar") {
        Some(false)
    } else {
        None
    }
}

pub(crate) fn plan_tar_extract(
    root: &SafeRoot,
    archive_path: &str,
    dest: &str,
    free_space: Option<u64>,
    gzip: bool,
) -> Result<TarExtractPlan, AppError> {
    let source = joined_file(root, archive_path)?;
    match node_at(source.as_path())? {
        Node::File => {}
        Node::Dir => return Err(invalid("không phải file nén")),
        Node::Missing => return Err(AppError::not_found(archive_path)),
    }
    let dest_path = join_user(root, dest)?;
    if node_at(dest_path.as_path())? == Node::File {
        return Err(exists(dest));
    }
    let reader = open_archive_reader(source.as_path(), gzip)?;
    let total = validate_tar_entries(root, dest, dest_path.as_path(), reader)?;
    if let Some(free) = free_space
        && total.saturating_add(DISK_MARGIN) > free
    {
        return Err(AppError::new(
            ErrorCode::NotEnoughDisk,
            format!("cần {total} byte, còn {free} byte"),
        ));
    }
    let real_root = fs::canonicalize(root.path()).map_err(|err| io_err(root.path(), err))?;
    Ok(TarExtractPlan {
        source: source.into_path_buf(),
        gzip,
        root: real_root,
        dest: dest_path.into_path_buf(),
        total,
    })
}

fn validate_tar_entries(
    root: &SafeRoot,
    dest: &str,
    dest_path: &Path,
    reader: Box<dyn Read>,
) -> Result<u64, AppError> {
    let mut archive = Archive::new(reader);
    let mut seen_files = HashSet::new();
    let (mut count, mut total) = (0usize, 0u64);
    for entry in archive.entries().map_err(tar_read_err)? {
        let entry = entry.map_err(tar_read_err)?;
        count += 1;
        if count > EXTRACT_MAX_ENTRIES {
            return Err(invalid("file nén có quá nhiều mục"));
        }
        let kind = entry.header().entry_type();
        if !matches!(
            kind,
            EntryType::Directory | EntryType::Regular | EntryType::Continuous
        ) {
            continue;
        }
        let raw_path = entry.path().map_err(tar_read_err)?;
        let Some(rel) = slash_path(&raw_path) else {
            // Bỏ qua mục gốc `.` / `./` trong tar; còn lại như `..` hoặc đường dẫn tuyệt đối thì chặn.
            if raw_path
                .components()
                .all(|c| matches!(c, std::path::Component::CurDir))
            {
                continue;
            }
            return Err(outside(&raw_path.display().to_string()));
        };
        let user_path = child_path(dest, &rel);
        let target = join_user(root, &user_path)?;
        if !target.as_path().starts_with(dest_path) || target.as_path() == dest_path {
            return Err(outside(&user_path));
        }
        let node = node_at(target.as_path())?;
        let is_file = kind != EntryType::Directory;
        if node == Node::File
            || (is_file && (node != Node::Missing || !seen_files.insert(user_path.clone())))
        {
            return Err(exists(&user_path).with_params(serde_json::json!({ "path": user_path })));
        }
        if is_file {
            total = total.saturating_add(entry.header().size().map_err(tar_read_err)?);
        }
    }
    Ok(total)
}

pub(crate) fn extract_tar_sync(
    plan: TarExtractPlan,
    progress: Progress<'_>,
) -> Result<(), AppError> {
    make_dirs_inside(&plan.root, &plan.dest)?;
    let reader = open_archive_reader(&plan.source, plan.gzip)?;
    let mut archive = Archive::new(reader);
    let mut done = 0u64;
    for entry in archive.entries().map_err(tar_read_err)? {
        let mut entry = entry.map_err(tar_read_err)?;
        let kind = entry.header().entry_type();
        if !matches!(
            kind,
            EntryType::Directory | EntryType::Regular | EntryType::Continuous
        ) {
            continue;
        }
        let rel_path = entry.path().map_err(tar_read_err)?.into_owned();
        let Some(rel) = slash_path(&rel_path) else {
            continue;
        };
        let target = plan.dest.join(&rel);
        if kind == EntryType::Directory {
            make_dirs_inside(&plan.root, &target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            make_dirs_inside(&plan.root, parent)?;
        }
        let size = entry.header().size().map_err(tar_read_err)?;
        let mut out = File::create_new(&target).map_err(|err| io_err(&target, err))?;
        let mut limited = (&mut entry).take(size);
        let mut buf = vec![0u8; CHUNK];
        loop {
            let n = limited.read(&mut buf).map_err(tar_read_err)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])
                .map_err(|err| io_err(&target, err))?;
            done += n as u64;
            progress(done)?;
        }
    }
    Ok(())
}

fn open_archive_reader(source: &Path, gzip: bool) -> Result<Box<dyn Read>, AppError> {
    let file = File::open(source).map_err(|err| io_err(source, err))?;
    let buf = BufReader::new(file);
    if gzip {
        Ok(Box::new(GzDecoder::new(buf)))
    } else {
        Ok(Box::new(buf))
    }
}

fn tar_read_err(err: io::Error) -> AppError {
    invalid(&format!("đọc file tar: {err}"))
}
