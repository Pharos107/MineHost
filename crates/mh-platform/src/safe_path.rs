use std::path::{Path, PathBuf};

use crate::PlatformError;

/// Tên thiết bị Windows không được dùng làm tên file. Chặn trên mọi OS để một server
/// tạo trên Linux vẫn chép sang Windows được.
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Thư mục gốc mà mọi đường dẫn nhận từ người dùng phải nằm bên trong.
#[derive(Debug, Clone)]
pub struct SafeRoot {
    root: PathBuf,
}

/// Một đường dẫn đã được kiểm tra là nằm trong [`SafeRoot`] tại thời điểm tạo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafePath(PathBuf);

impl SafeRoot {
    /// Thư mục gốc phải tồn tại; nó được chuẩn hóa để so sánh symlink cho đúng.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, PlatformError> {
        let root = root.as_ref();
        let root = root
            .canonicalize()
            .map_err(|e| PlatformError::io(root, e))?;
        Ok(Self { root })
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Ghép một đường dẫn tương đối do người dùng gửi lên vào thư mục gốc.
    ///
    /// Chấp nhận cả `/` lẫn `\`; dấu phân cách ở đầu được hiểu là tính từ thư mục gốc.
    /// Từ chối `..` thoát ra ngoài, ký tự `:` (ổ đĩa, ADS), tên thiết bị Windows, và
    /// symlink trỏ ra ngoài thư mục gốc.
    pub fn join(&self, user_path: &str) -> Result<SafePath, PlatformError> {
        let outside = || PlatformError::PathOutsideRoot(user_path.to_owned());
        let invalid = || PlatformError::InvalidPath(user_path.to_owned());

        if user_path.contains('\0') {
            return Err(invalid());
        }

        let mut parts: Vec<&str> = Vec::new();
        for part in user_path.split(['/', '\\']) {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop().ok_or_else(outside)?;
                }
                name if !is_valid_name(name) => return Err(invalid()),
                name => parts.push(name),
            }
        }

        let candidate = parts
            .iter()
            .fold(self.root.clone(), |acc, part| acc.join(part));
        self.check_symlinks(&candidate).map_err(|_| outside())?;
        Ok(SafePath(candidate))
    }

    /// Phần đã tồn tại của đường dẫn có thể chứa symlink; chuẩn hóa nó rồi kiểm tra lại.
    fn check_symlinks(&self, candidate: &Path) -> Result<(), ()> {
        let existing = candidate.ancestors().find(|p| p.exists()).ok_or(())?;
        let resolved = existing.canonicalize().map_err(|_| ())?;
        if resolved.starts_with(&self.root) {
            Ok(())
        } else {
            Err(())
        }
    }
}

impl SafePath {
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

impl AsRef<Path> for SafePath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

pub fn is_valid_name(name: &str) -> bool {
    if name.contains(':') || name.chars().any(char::is_control) {
        return false;
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    !RESERVED_NAMES
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> (tempfile::TempDir, SafeRoot) {
        let tmp = tempfile::tempdir().unwrap();
        let root = SafeRoot::new(tmp.path()).unwrap();
        (tmp, root)
    }

    #[test]
    fn joins_plain_relative_paths() {
        let (_tmp, root) = root();
        let path = root.join("world/level.dat").unwrap();
        assert_eq!(path.as_path(), root.path().join("world").join("level.dat"));
    }

    #[test]
    fn accepts_backslashes_and_dot_segments() {
        let (_tmp, root) = root();
        let path = root.join(r"world\.\region\..\level.dat").unwrap();
        assert_eq!(path.as_path(), root.path().join("world").join("level.dat"));
    }

    #[test]
    fn leading_separator_is_relative_to_root() {
        let (_tmp, root) = root();
        let path = root.join("/server.properties").unwrap();
        assert_eq!(path.as_path(), root.path().join("server.properties"));
    }

    #[test]
    fn empty_path_is_the_root_itself() {
        let (_tmp, root) = root();
        assert_eq!(root.join("").unwrap().as_path(), root.path());
    }

    #[test]
    fn rejects_parent_escape() {
        let (_tmp, root) = root();
        for bad in ["..", "../x", "a/../../x", r"a\..\..\x"] {
            assert!(
                matches!(root.join(bad), Err(PlatformError::PathOutsideRoot(_))),
                "phải chặn {bad}"
            );
        }
    }

    #[test]
    fn rejects_drive_letters_and_streams() {
        let (_tmp, root) = root();
        for bad in [r"C:\Windows", "C:", "file.txt:stream", "a\0b"] {
            assert!(
                matches!(root.join(bad), Err(PlatformError::InvalidPath(_))),
                "phải chặn {bad}"
            );
        }
    }

    #[test]
    fn rejects_windows_device_names() {
        let (_tmp, root) = root();
        for bad in ["nul", "CON.txt", "logs/com1.log", "aux "] {
            assert!(
                matches!(root.join(bad), Err(PlatformError::InvalidPath(_))),
                "phải chặn {bad}"
            );
        }
        assert!(root.join("console.log").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_pointing_outside() {
        let (_tmp, root) = root();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();

        assert!(matches!(
            root.join("link/secret"),
            Err(PlatformError::PathOutsideRoot(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn allows_symlink_pointing_inside() {
        let (_tmp, root) = root();
        std::fs::create_dir(root.path().join("real")).unwrap();
        std::os::unix::fs::symlink(root.path().join("real"), root.path().join("link")).unwrap();

        assert!(root.join("link/file").is_ok());
    }
}
