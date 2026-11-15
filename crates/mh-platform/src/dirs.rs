use std::fs;
use std::path::{Path, PathBuf};

use crate::PlatformError;

/// Tên thư mục dữ liệu bên trong thư mục ứng dụng của OS.
const APP_DIR_NAME: &str = "minehost";

/// Các thư mục con trong thư mục dữ liệu của MineHost.
///
/// Mặc định: Windows `%LOCALAPPDATA%\minehost`, Linux `~/.local/share/minehost`.
#[derive(Debug, Clone)]
pub struct DataDirs {
    root: PathBuf,
}

impl DataDirs {
    /// Dùng `override_root` nếu có, nếu không thì lấy thư mục mặc định theo OS.
    /// Đường dẫn tương đối được đổi thành tuyệt đối theo thư mục hiện tại.
    pub fn resolve(override_root: Option<PathBuf>) -> Result<Self, PlatformError> {
        let root = match override_root {
            Some(path) => path,
            None => default_root()?,
        };
        let root = std::path::absolute(&root).map_err(|e| PlatformError::io(&root, e))?;
        Ok(Self { root })
    }

    /// Tạo thư mục gốc và mọi thư mục con nếu chưa có.
    pub fn ensure(&self) -> Result<(), PlatformError> {
        for dir in [
            self.root.clone(),
            self.logs(),
            self.servers(),
            self.java(),
            self.cache(),
            self.backups(),
            self.trash(),
        ] {
            fs::create_dir_all(&dir).map_err(|e| PlatformError::io(&dir, e))?;
        }
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    /// Hash mật khẩu admin và cờ LAN (spec P2, mục 4.3).
    pub fn auth_file(&self) -> PathBuf {
        self.root.join("auth.toml")
    }

    /// API key bí mật của người dùng, ví dụ CurseForge (spec P3, mục 4.3).
    pub fn secrets_file(&self) -> PathBuf {
        self.root.join("secrets.toml")
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn servers(&self) -> PathBuf {
        self.root.join("servers")
    }

    pub fn java(&self) -> PathBuf {
        self.root.join("java")
    }

    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }

    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }

    /// Server bị xóa được chuyển vào đây, vì chưa có backup thì không được xóa vĩnh viễn.
    pub fn trash(&self) -> PathBuf {
        self.root.join("trash")
    }
}

fn default_root() -> Result<PathBuf, PlatformError> {
    let base = directories::BaseDirs::new().ok_or(PlatformError::NoHomeDir)?;
    Ok(base.data_local_dir().join(APP_DIR_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_creates_all_subdirectories() {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = DataDirs::resolve(Some(tmp.path().join("data"))).unwrap();
        dirs.ensure().unwrap();

        for dir in [
            dirs.logs(),
            dirs.servers(),
            dirs.java(),
            dirs.cache(),
            dirs.backups(),
            dirs.trash(),
        ] {
            assert!(dir.is_dir(), "thiếu thư mục {}", dir.display());
        }
        assert_eq!(dirs.config_file(), dirs.root().join("config.toml"));
        assert_eq!(dirs.auth_file(), dirs.root().join("auth.toml"));
        assert_eq!(dirs.secrets_file(), dirs.root().join("secrets.toml"));
    }

    #[test]
    fn relative_override_becomes_absolute() {
        let dirs = DataDirs::resolve(Some(PathBuf::from("some-relative-dir"))).unwrap();
        assert!(dirs.root().is_absolute());
    }

    #[test]
    fn default_root_ends_with_app_dir_name() {
        let dirs = DataDirs::resolve(None).unwrap();
        assert!(dirs.root().ends_with(APP_DIR_NAME));
    }
}
