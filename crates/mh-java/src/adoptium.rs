//! API của Adoptium: link tải JRE Temurin mới nhất cho một bản Java.

use std::time::Duration;

use mh_net::{Checksum, HttpClient};
use serde::Deserialize;

use crate::JavaError;

pub const API_BASE: &str = "https://api.adoptium.net";
/// Temurin ra bản vá vài lần mỗi năm; không cần hỏi lại thường xuyên.
const ASSETS_TTL: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArchiveKind {
    Zip,
    TarGz,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Package {
    pub link: String,
    pub checksum: Checksum,
    pub size: u64,
    pub kind: ArchiveKind,
}

#[derive(Deserialize)]
struct RawAsset {
    binary: RawBinary,
}

#[derive(Deserialize)]
struct RawBinary {
    package: RawPackage,
}

#[derive(Deserialize)]
struct RawPackage {
    name: String,
    link: String,
    checksum: String,
    size: u64,
}

/// Tên OS và kiến trúc theo cách Adoptium gọi. P1 chỉ hỗ trợ Windows và Linux.
pub(crate) fn platform() -> Option<(&'static str, &'static str)> {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "linux" => "linux",
        _ => return None,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        _ => return None,
    };
    Some((os, arch))
}

pub(crate) async fn latest_package(
    http: &HttpClient,
    api_base: &str,
    major: u16,
) -> Result<Package, JavaError> {
    let not_available = |(os, arch)| JavaError::NotAvailable { major, os, arch };
    let (os, arch) = platform().ok_or(not_available((std::env::consts::OS, "?")))?;
    let url = format!(
        "{api_base}/v3/assets/latest/{major}/hotspot\
         ?architecture={arch}&image_type=jre&os={os}&vendor=eclipse"
    );
    let assets: Vec<RawAsset> = http.get_json(&url, ASSETS_TTL).await?.value;
    let raw = assets
        .into_iter()
        .next()
        .ok_or(not_available((os, arch)))?
        .binary
        .package;
    parse_package(raw)
}

fn parse_package(raw: RawPackage) -> Result<Package, JavaError> {
    // Chọn cách giải nén theo tên file chứ không theo OS, để test được cả hai loại ở mọi máy.
    let kind = if raw.name.ends_with(".zip") {
        ArchiveKind::Zip
    } else if raw.name.ends_with(".tar.gz") {
        ArchiveKind::TarGz
    } else {
        return Err(JavaError::install(format!(
            "định dạng file lạ: {}",
            raw.name
        )));
    };
    let checksum = Checksum::sha256(&raw.checksum)
        .ok_or_else(|| JavaError::install("sha256 từ Adoptium sai định dạng"))?;
    Ok(Package {
        link: raw.link,
        checksum,
        size: raw.size,
        kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(os: &str) -> RawPackage {
        let path = format!(
            "{}/fixtures/adoptium-21-{os}-x64.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let text = std::fs::read_to_string(path).unwrap();
        let mut assets: Vec<RawAsset> = serde_json::from_str(&text).unwrap();
        assets.remove(0).binary.package
    }

    #[test]
    fn windows_package_is_zip() {
        let pkg = parse_package(fixture("windows")).unwrap();
        assert_eq!(pkg.kind, ArchiveKind::Zip);
        assert_eq!(pkg.size, 48_999_141);
        assert!(pkg.checksum.hex().starts_with("d35f31e7"));
        assert!(pkg.link.starts_with("https://github.com/adoptium/"));
    }

    #[test]
    fn linux_package_is_tar_gz() {
        let pkg = parse_package(fixture("linux")).unwrap();
        assert_eq!(pkg.kind, ArchiveKind::TarGz);
    }

    #[test]
    fn unknown_archive_format_is_rejected() {
        let mut raw = fixture("linux");
        raw.name = "jre.msi".to_owned();
        assert!(matches!(parse_package(raw), Err(JavaError::Install(_))));
    }

    #[test]
    fn current_machine_is_supported() {
        assert!(platform().is_some());
    }
}
