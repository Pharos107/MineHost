use std::io::Write;
use std::path::Path;

use crate::PlatformError;

/// Ghi file theo kiểu nguyên tử: ghi vào file tạm cùng thư mục, `fsync`, rồi đổi tên đè lên.
///
/// Nếu mất điện giữa chừng thì file cũ vẫn còn nguyên, không bao giờ bị ghi dở.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), PlatformError> {
    let dir = path
        .parent()
        .ok_or_else(|| PlatformError::InvalidPath(path.display().to_string()))?;

    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(|e| PlatformError::io(dir, e))?;
    tmp.write_all(contents)
        .map_err(|e| PlatformError::io(tmp.path(), e))?;
    tmp.as_file()
        .sync_all()
        .map_err(|e| PlatformError::io(tmp.path(), e))?;
    tmp.persist(path)
        .map_err(|e| PlatformError::io(path, e.error))?;
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
}
