use std::io::Write;
use std::path::Path;

use crate::PlatformError;

/// Ghi file theo kiểu nguyên tử: ghi vào file tạm cùng thư mục, `fsync`, rồi đổi tên đè lên.
///
/// Nếu mất điện giữa chừng thì file cũ vẫn còn nguyên, không bao giờ bị ghi dở.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), PlatformError> {
    write_atomic_inner(path, contents, false)
}

/// Như [`write_atomic`], nhưng đặt quyền `0600` (trên Unix) cho file tạm trước khi ghi nội dung
/// rồi mới đổi tên, để nội dung bí mật không bao giờ nằm trên đĩa với quyền rộng hơn.
pub fn write_atomic_private(path: &Path, contents: &[u8]) -> Result<(), PlatformError> {
    write_atomic_inner(path, contents, true)
}

fn write_atomic_inner(path: &Path, contents: &[u8], private: bool) -> Result<(), PlatformError> {
    let dir = path
        .parent()
        .ok_or_else(|| PlatformError::InvalidPath(path.display().to_string()))?;

    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(|e| PlatformError::io(dir, e))?;
    if private {
        set_private_mode(tmp.path(), tmp.as_file())?;
    }
    tmp.write_all(contents)
        .map_err(|e| PlatformError::io(tmp.path(), e))?;
    tmp.as_file()
        .sync_all()
        .map_err(|e| PlatformError::io(tmp.path(), e))?;
    tmp.persist(path)
        .map_err(|e| PlatformError::io(path, e.error))?;
    Ok(())
}

#[cfg(unix)]
fn set_private_mode(path: &Path, file: &std::fs::File) -> Result<(), PlatformError> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(|e| PlatformError::io(path, e))
}

#[cfg(not(unix))]
fn set_private_mode(_path: &Path, _file: &std::fs::File) -> Result<(), PlatformError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_new_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.toml");
        write_atomic(&path, b"x = 1").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"x = 1");
    }

    #[test]
    fn replaces_existing_file_and_leaves_no_temp_files() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.toml");
        write_atomic(&path, b"old").unwrap();
        write_atomic(&path, b"new").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        let entries = std::fs::read_dir(tmp.path()).unwrap().count();
        assert_eq!(entries, 1);
    }

    #[test]
    fn fails_when_directory_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("missing").join("a.toml");
        assert!(write_atomic(&path, b"x").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_private_sets_mode_600_even_over_644() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("secrets.toml");
        std::fs::write(&path, b"old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        write_atomic_private(&path, b"secret = true").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(std::fs::read(&path).unwrap(), b"secret = true");
    }
}
