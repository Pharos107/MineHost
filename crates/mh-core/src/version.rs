//! Phiên bản Minecraft và loại server.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::InvalidValue;

/// Phiên bản Minecraft, ví dụ `1.21.4`, `26.3`, `26.4-snapshot-5`.
///
/// Chỉ là chuỗi đã kiểm tra, không phân tích thành số: Mojang đổi cách đánh số nhiều lần,
/// nên thứ tự lấy từ `releaseTime` trong manifest (spec P1, mục 3.5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct McVersion(String);

impl McVersion {
    const MAX_LEN: usize = 64;

    /// Cho phép chữ, số ASCII và `. - _ +`; khoảng trắng chỉ ở giữa, vì vài bản pre-release
    /// cũ của Mojang có tên kiểu `1.14 Pre-Release 1`.
    pub fn parse(s: &str) -> Result<Self, InvalidValue> {
        let err = |reason| Err(InvalidValue::new("McVersion", reason));
        if s.is_empty() || s.len() > Self::MAX_LEN {
            return err("rỗng hoặc dài quá 64 ký tự");
        }
        let starts_ok = s.starts_with(|c: char| c.is_ascii_alphanumeric());
        let ends_ok = s.ends_with(|c: char| c.is_ascii_alphanumeric());
        if !starts_ok || !ends_ok {
            return err("phải bắt đầu và kết thúc bằng chữ hoặc số");
        }
        let allowed =
            |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+' | ' ');
        if !s.chars().all(allowed) {
            return err("có ký tự không cho phép");
        }
        Ok(Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for McVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for McVersion {
    type Err = InvalidValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for McVersion {
    type Error = InvalidValue;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<McVersion> for String {
    fn from(v: McVersion) -> Self {
        v.0
    }
}

/// Loại server. P1 có Paper; P2 thêm Vanilla và Purpur (spec P2, mục 5.4); P3 thêm bốn mod
/// loader (spec P3, mục 3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LoaderKind {
    Paper,
    Vanilla,
    Purpur,
    Fabric,
    Quilt,
    Forge,
    #[serde(rename = "neoforge")]
    NeoForge,
}

/// Server nhận thêm loại nội dung nào: mod hay plugin (spec P3, mục 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    Mods,
    Plugins,
}

impl ContentKind {
    /// Thư mục trong `server/` chứa nội dung đang bật.
    pub const fn dir(self) -> &'static str {
        match self {
            Self::Mods => "mods",
            Self::Plugins => "plugins",
        }
    }

    /// Thư mục trong `server/` chứa nội dung đang tắt.
    pub const fn disabled_dir(self) -> &'static str {
        match self {
            Self::Mods => "mods-disabled",
            Self::Plugins => "plugins-disabled",
        }
    }
}

impl LoaderKind {
    pub const ALL: [Self; 7] = [
        Self::Paper,
        Self::Vanilla,
        Self::Purpur,
        Self::Fabric,
        Self::Quilt,
        Self::Forge,
        Self::NeoForge,
    ];

    /// Chuỗi giống hệt giá trị serde.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Paper => "paper",
            Self::Vanilla => "vanilla",
            Self::Purpur => "purpur",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
        }
    }

    pub fn parse(s: &str) -> Result<Self, InvalidValue> {
        Self::ALL
            .into_iter()
            .find(|l| l.as_str() == s)
            .ok_or(InvalidValue::new("LoaderKind", "loại server chưa hỗ trợ"))
    }

    /// Vanilla không nhận mod hay plugin nên không có trang Mod.
    pub const fn content_kind(self) -> Option<ContentKind> {
        match self {
            Self::Paper | Self::Purpur => Some(ContentKind::Plugins),
            Self::Fabric | Self::Quilt | Self::Forge | Self::NeoForge => Some(ContentKind::Mods),
            Self::Vanilla => None,
        }
    }

    /// Loader cài bằng installer và có `game.loader_version` (spec P3, mục 4.1).
    pub const fn is_mod_loader(self) -> bool {
        matches!(self.content_kind(), Some(ContentKind::Mods))
    }
}

/// Phiên bản của mod loader, ví dụ `0.16.9`, `21.4.140`, `26.1.2.76-beta`, `47.3.0`.
///
/// Như `McVersion`, chỉ là chuỗi đã kiểm tra; thứ tự lấy từ nguồn của loader, không so sánh chuỗi.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct LoaderVersion(String);

impl LoaderVersion {
    const MAX_LEN: usize = 64;

    pub fn parse(s: &str) -> Result<Self, InvalidValue> {
        let err = |reason| Err(InvalidValue::new("LoaderVersion", reason));
        if s.is_empty() || s.len() > Self::MAX_LEN {
            return err("rỗng hoặc dài quá 64 ký tự");
        }
        let edges_ok = s.starts_with(|c: char| c.is_ascii_alphanumeric())
            && s.ends_with(|c: char| c.is_ascii_alphanumeric());
        let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+');
        if !edges_ok || !s.chars().all(allowed) {
            return err("chỉ được có chữ, số và . - _ +, đầu và cuối là chữ hoặc số");
        }
        Ok(Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LoaderVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for LoaderVersion {
    type Error = InvalidValue;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<LoaderVersion> for String {
    fn from(v: LoaderVersion) -> Self {
        v.0
    }
}

impl fmt::Display for LoaderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LoaderKind {
    type Err = InvalidValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Bản Minecraft mà một bản NeoForge nhắm tới.
///
/// Theo quy ước đánh số của NeoForge: `20.2.x` trở lên tương ứng `1.20.2` (`21.0.x` là `1.21`),
/// còn từ năm `26` dùng số năm trực tiếp (`26.1.0.x` → `26.1`, `26.1.2.x` → `26.1.2`).
/// Bản có hậu tố `+...` hoặc `alpha` không phải bản release ổn định nên trả `None`.
pub fn neoforge_minecraft(version: &str) -> Option<String> {
    if version.contains('+') || version.contains("alpha") {
        return None;
    }
    let core = version.split('-').next()?;
    let parts: Vec<u32> = core
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    match parts.as_slice() {
        [major @ 20..=25, minor, _] if (*major, *minor) >= (20, 2) => Some(match minor {
            0 => format!("1.{major}"),
            _ => format!("1.{major}.{minor}"),
        }),
        [year, drop, 0, _] if *year >= 26 => Some(format!("{year}.{drop}")),
        [year, drop, hotfix, _] if *year >= 26 => Some(format!("{year}.{drop}.{hotfix}")),
        _ => None,
    }
}

/// Các bản Java chọn được cho từng server (spec P3, mục 3.10): mọi bản Mojang từng yêu cầu
/// (8, 16, 17, 21, 25) cộng LTS 11. Đều có trên Adoptium.
pub const JAVA_MAJORS: &[u16] = &[8, 11, 16, 17, 21, 25];

/// Có nằm trong danh sách phiên bản Java MineHost hỗ trợ không.
pub fn is_supported_java_major(major: u16) -> bool {
    JAVA_MAJORS.contains(&major)
}

/// Bản Java Mojang yêu cầu (`javaVersion.majorVersion` trong file phiên bản) cho một bản release.
///
/// Dùng để cảnh báo khi người dùng chọn bản khác, kể cả sau khi đã đổi `java_major`. Các mốc đã cố
/// định: dưới 1.17 là 8, 1.17 là 16, 1.18 tới 1.20.4 là 17, 1.20.5 tới 1.21.x là 21, 26.x là 25.
/// Bản chưa biết (snapshot, năm sau 26) trả `None`.
pub fn mojang_java_major(minecraft: &McVersion) -> Option<u16> {
    let parts: Vec<u32> = minecraft
        .as_str()
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    match parts.as_slice() {
        [1, minor, rest @ ..] => {
            let patch = rest.first().copied().unwrap_or(0);
            Some(match (*minor, patch) {
                (..=16, _) => 8,
                (17, _) => 16,
                (18..=19, _) | (20, ..=4) => 17,
                (20, _) | (21, _) => 21,
                _ => return None,
            })
        }
        [26, ..] => Some(25),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mc_version_accepts_mojang_names() {
        let names = [
            "1.21.4",
            "26.3",
            "1.21.11",
            "26.4-snapshot-5",
            "24w14a",
            "1.14 Pre-Release 1",
            "b1.7.3",
        ];
        for ok in names {
            assert_eq!(McVersion::parse(ok).unwrap().as_str(), ok);
        }
    }

    #[test]
    fn mc_version_rejects_bad_strings() {
        let long = "1".repeat(65);
        let bad = [
            "",
            " 1.21",
            "1.21 ",
            "../1.21",
            "1.21/x",
            ".",
            "1.21\n",
            "phiên-bản",
            long.as_str(),
        ];
        for s in bad {
            assert!(McVersion::parse(s).is_err(), "{s:?}");
        }
        assert!(serde_json::from_str::<McVersion>("\"..\"").is_err());
    }

    #[test]
    fn loader_kind_matches_serde_name() {
        for kind in LoaderKind::ALL {
            let name = kind.as_str();
            assert_eq!(LoaderKind::parse(name).unwrap(), kind);
            assert_eq!(serde_json::to_string(&kind).unwrap(), format!("\"{name}\""));
            assert_eq!(
                serde_json::from_str::<LoaderKind>(&format!("\"{name}\"")).unwrap(),
                kind
            );
        }
        assert!(LoaderKind::parse("Paper").is_err());
        assert!(LoaderKind::parse("neo_forge").is_err());
        assert!(serde_json::from_str::<LoaderKind>("\"folia\"").is_err());
    }

    #[test]
    fn content_kind_per_loader() {
        assert_eq!(LoaderKind::Vanilla.content_kind(), None);
        assert_eq!(LoaderKind::Paper.content_kind(), Some(ContentKind::Plugins));
        assert_eq!(LoaderKind::NeoForge.content_kind(), Some(ContentKind::Mods));
        assert!(LoaderKind::Quilt.is_mod_loader());
        assert!(!LoaderKind::Purpur.is_mod_loader());
        assert_eq!(ContentKind::Plugins.dir(), "plugins");
        assert_eq!(ContentKind::Mods.disabled_dir(), "mods-disabled");
        assert_eq!(ContentKind::Plugins.disabled_dir(), "plugins-disabled");
    }

    #[test]
    fn mojang_java_major_follows_release_milestones() {
        let cases = [
            ("1.12.2", Some(8)),
            ("1.16.5", Some(8)),
            ("1.17", Some(16)),
            ("1.17.1", Some(16)),
            ("1.18", Some(17)),
            ("1.20.4", Some(17)),
            ("1.20.5", Some(21)),
            ("1.21.4", Some(21)),
            ("26.1", Some(25)),
            ("26.1.2", Some(25)),
            ("24w14a", None),
            ("27.1", None),
        ];
        for (mc, java) in cases {
            let version = McVersion::parse(mc).unwrap();
            assert_eq!(mojang_java_major(&version), java, "{mc}");
            if let Some(major) = java {
                assert!(is_supported_java_major(major), "{mc} cần Java {major}");
            }
        }
    }

    #[test]
    fn loader_version_accepts_real_versions() {
        for ok in [
            "0.16.9",
            "21.4.140",
            "26.1.2.76-beta",
            "47.3.0",
            "1.20.1-47.3.0",
            "0.27.1+mc",
        ] {
            assert_eq!(LoaderVersion::parse(ok).unwrap().as_str(), ok);
        }
        for bad in [
            "", "-1", "1.0-", "1 0", "../1", "1/2", "1.0
",
        ] {
            assert!(LoaderVersion::parse(bad).is_err(), "{bad:?}");
        }
    }
}
