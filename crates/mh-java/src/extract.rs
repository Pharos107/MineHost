//! Giải nén JRE. Mọi đường dẫn trong archive đi qua `SafeRoot` để chặn zip-slip; symlink bị bỏ qua.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use mh_platform::{PlatformError, SafeRoot};

use crate::JavaError;
use crate::adoptium::ArchiveKind;

/// Giải nén `archive` vào `dest` rồi trả về thư mục gốc thật của JRE: archive của Temurin
/// bọc mọi thứ trong một thư mục kiểu `jdk-21.0.5+11-jre/`, thư mục đó được bỏ qua.
pub(crate) fn unpack(kind: ArchiveKind, archive: &Path, dest: &Path) -> Result<PathBuf, JavaError> {
    fs::create_dir_all(dest).map_err(JavaError::install)?;
    let root = SafeRoot::new(dest).map_err(JavaError::install)?;
    match kind {
        ArchiveKind::Zip => unzip(archive, &root)?,
        ArchiveKind::TarGz => untar(archive, &root)?,
    }
    single_root(dest)
}

fn unzip(archive: &Path, root: &SafeRoot) -> Result<(), JavaError> {
    let file = File::open(archive).map_err(JavaError::install)?;
    let mut zip = zip::ZipArchive::new(file).map_err(JavaError::install)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(JavaError::install)?;
        let name = entry.name().map_err(JavaError::install)?.into_owned();
        if entry.is_symlink() {
            tracing::debug!(name, "bỏ qua symlink trong file nén Java");
            continue;
        }
        let target = safe_join(root, &name)?;
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(JavaError::install)?;
            continue;
        }
        let mode = entry.unix_mode();
        write_file(&target, &mut entry, mode)?;
    }
    Ok(())
}

fn untar(archive: &Path, root: &SafeRoot) -> Result<(), JavaError> {
    let file = File::open(archive).map_err(JavaError::install)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    for entry in tar.entries().map_err(JavaError::install)? {
        let mut entry = entry.map_err(JavaError::install)?;
        let name = entry
            .path()
            .map_err(JavaError::install)?
            .to_string_lossy()
            .into_owned();
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            fs::create_dir_all(safe_join(root, &name)?).map_err(JavaError::install)?;
        } else if kind.is_file() {
            let mode = entry.header().mode().ok();
            write_file(&safe_join(root, &name)?, &mut entry, mode)?;
        } else {
            tracing::debug!(
                name,
                "bỏ qua symlink hoặc file đặc biệt trong file nén Java"
            );
        }
    }
    Ok(())
}

fn safe_join(root: &SafeRoot, name: &str) -> Result<PathBuf, JavaError> {
    root.join(name)
        .map(|p| p.into_path_buf())
        .map_err(|e| match e {
            PlatformError::PathOutsideRoot(_) | PlatformError::InvalidPath(_) => {
                JavaError::UnsafeArchive(name.to_owned())
            }
            other => JavaError::install(other),
        })
}

fn write_file(target: &Path, data: &mut impl io::Read, mode: Option<u32>) -> Result<(), JavaError> {
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).map_err(JavaError::install)?;
    }
    let mut out = File::create(target).map_err(JavaError::install)?;
    io::copy(data, &mut out).map_err(JavaError::install)?;
    set_mode(target, mode)
}

/// Giữ quyền chạy của `bin/java` trên Linux.
#[cfg(unix)]
fn set_mode(target: &Path, mode: Option<u32>) -> Result<(), JavaError> {
    use std::os::unix::fs::PermissionsExt;
    let Some(mode) = mode else { return Ok(()) };
    fs::set_permissions(target, fs::Permissions::from_mode(mode & 0o777))
        .map_err(JavaError::install)
}

#[cfg(not(unix))]
fn set_mode(_target: &Path, _mode: Option<u32>) -> Result<(), JavaError> {
    Ok(())
}

fn single_root(dest: &Path) -> Result<PathBuf, JavaError> {
    let entries: Vec<_> = fs::read_dir(dest)
        .map_err(JavaError::install)?
        .collect::<Result<_, _>>()
        .map_err(JavaError::install)?;
    match entries.as_slice() {
        [only] if only.path().is_dir() => Ok(only.path()),
        _ => Ok(dest.to_path_buf()),
    }
}

#[cfg(test)]
mod tests;
