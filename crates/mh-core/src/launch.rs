//! Cách chạy server sau khi cài (spec P3, mục 3.2).
//!
//! Paper, Vanilla, Fabric, Quilt và Forge dưới 1.17 chạy bằng `-jar`. Forge từ 1.17 và NeoForge
//! chạy bằng file tham số mà installer tạo trong `libraries/`, gọi qua `@file` của Java.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::InvalidValue;
use crate::instance::JarName;

/// Chỗ đặt tên OS trong `args_file`, để server tạo trên Linux chép sang Windows vẫn chạy.
pub const OS_PLACEHOLDER: &str = "{os}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    /// `java … -jar <jar> <args>`.
    Jar(JarName),
    /// `java … @<file> <args>`.
    ArgsFile(ArgsFilePath),
}

/// Họ OS theo cách installer Forge, NeoForge đặt tên file tham số.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgsOs {
    Unix,
    Windows,
}

impl ArgsOs {
    /// OS của bản build đang chạy.
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Unix
        }
    }

    const fn file_prefix(self) -> &'static str {
        match self {
            Self::Unix => "unix",
            Self::Windows => "win",
        }
    }
}

/// Đường dẫn tương đối tới file tham số, dùng `/`, ví dụ
/// `libraries/net/neoforged/neoforge/21.4.140/{os}_args.txt`.
///
/// Không chứa `..`, không tuyệt đối, chỉ có ký tự an toàn, nên ghép vào thư mục server không
/// thoát ra ngoài được. Lúc chạy vẫn phải đi qua `SafeRoot` như mọi đường dẫn khác.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArgsFilePath(String);

impl ArgsFilePath {
    const MAX_LEN: usize = 256;

    pub fn parse(s: &str) -> Result<Self, InvalidValue> {
        let err = |reason| Err(InvalidValue::new("ArgsFilePath", reason));
        if s.is_empty() || s.len() > Self::MAX_LEN {
            return err("rỗng hoặc dài quá 256 ký tự");
        }
        if !s.ends_with(".txt") {
            return err("không có đuôi .txt");
        }
        let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+');
        let rest = s.replace(OS_PLACEHOLDER, "os");
        for part in rest.split('/') {
            if part.is_empty() || part == "." || part == ".." {
                return err("có đoạn rỗng, `.` hoặc `..`");
            }
            if !part.chars().all(allowed) {
                return err("có ký tự không cho phép");
            }
        }
        Ok(Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Đường dẫn thật cho `os`, đã thay `{os}`.
    pub fn for_os(&self, os: ArgsOs) -> String {
        self.0.replace(OS_PLACEHOLDER, os.file_prefix())
    }
}

impl fmt::Display for ArgsFilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for ArgsFilePath {
    type Error = InvalidValue;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<ArgsFilePath> for String {
    fn from(p: ArgsFilePath) -> Self {
        p.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_installer_paths() {
        let p = ArgsFilePath::parse("libraries/net/neoforged/neoforge/21.4.140/{os}_args.txt")
            .unwrap();
        assert_eq!(
            p.for_os(ArgsOs::Unix),
            "libraries/net/neoforged/neoforge/21.4.140/unix_args.txt"
        );
        assert_eq!(
            p.for_os(ArgsOs::Windows),
            "libraries/net/neoforged/neoforge/21.4.140/win_args.txt"
        );
        let forge = "libraries/net/minecraftforge/forge/1.20.1-47.3.0/{os}_args.txt";
        assert!(ArgsFilePath::parse(forge).is_ok());
    }

    #[test]
    fn rejects_paths_that_escape() {
        for bad in [
            "",
            "../args.txt",
            "/etc/args.txt",
            "C:/args.txt",
            "libraries//args.txt",
            "libraries/./args.txt",
            "libraries\\args.txt",
            "libraries/args.sh",
            "lib raries/args.txt",
        ] {
            assert!(ArgsFilePath::parse(bad).is_err(), "{bad}");
        }
    }
}
