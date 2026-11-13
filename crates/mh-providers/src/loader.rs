//! Kiểu chung của mod loader: bản loader và kế hoạch cài (spec P3, mục 3.1; ADR 0006).
//!
//! Provider chỉ trả kế hoạch: file cần tải, lệnh installer, cách chạy server. `mh-app` tải file,
//! chạy installer qua `mh-runtime` rồi kiểm tra kết quả trên đĩa.

use std::cmp::Ordering;

use mh_core::{Launch, LoaderVersion, McVersion};
use mh_net::{Checksum, HostAllowlist};

/// Thư mục tạm trong `server/` chứa installer; `mh-app` xóa sau khi cài.
pub const INSTALL_DIR: &str = ".minehost-install";

/// Một bản loader cho một phiên bản Minecraft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderRelease {
    pub version: LoaderVersion,
    /// Loader tự đánh dấu là ổn định; bản đề xuất luôn đứng đầu danh sách.
    pub stable: bool,
}

/// File cần tải trước khi chạy installer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    pub url: String,
    pub checksum: Checksum,
    pub size: Option<u64>,
    /// Đường dẫn tương đối với `server/`, dùng `/`.
    pub dest: String,
    pub hosts: HostAllowlist,
}

/// Lệnh `java -jar <jar> <args>` chạy trong `server/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallerRun {
    /// Đường dẫn tương đối với `server/`, là `dest` của một file trong `files`.
    pub jar: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    pub minecraft: McVersion,
    pub loader_version: LoaderVersion,
    pub files: Vec<PlannedFile>,
    pub installer: Option<InstallerRun>,
    /// Cách chạy dự kiến. Với `ArgsFile`, `mh-app` phải thấy file thật trên đĩa sau khi cài.
    pub launch: Launch,
}

/// Bản đề xuất (ổn định đầu tiên theo thứ tự nguồn trả) lên đầu, phần còn lại giữ nguyên thứ tự.
pub(crate) fn recommended_first(mut releases: Vec<LoaderRelease>) -> Vec<LoaderRelease> {
    if let Some(i) = releases.iter().position(|r| r.stable) {
        let pick = releases.remove(i);
        releases.insert(0, pick);
    }
    releases
}

/// Chọn bản người dùng hỏi, hoặc bản đề xuất.
pub(crate) fn pick<'a>(
    releases: &'a [LoaderRelease],
    wanted: Option<&LoaderVersion>,
) -> Option<&'a LoaderRelease> {
    match wanted {
        Some(v) => releases.iter().find(|r| &r.version == v),
        None => releases.first(),
    }
}

/// So sánh kiểu semver đủ dùng cho bản loader: phần số trước `-` so theo số, bản không có
/// hậu tố (ổn định) đứng trên bản `-beta…` cùng số, hậu tố so từng đoạn.
pub(crate) fn compare_versions(a: &str, b: &str) -> Ordering {
    let (a_core, a_pre) = split_pre(a);
    let (b_core, b_pre) = split_pre(b);
    compare_parts(a_core, b_core).then_with(|| match (a_pre, b_pre) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => compare_parts(x, y),
    })
}

fn split_pre(v: &str) -> (&str, Option<&str>) {
    match v.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (v, None),
    }
}

fn compare_parts(a: &str, b: &str) -> Ordering {
    let mut xs = a.split('.');
    let mut ys = b.split('.');
    loop {
        match (xs.next(), ys.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    _ => x.cmp(y),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_sort_like_semver() {
        let mut v = vec![
            "0.20.0-beta.9",
            "0.24.0",
            "0.20.0-beta.10",
            "0.30.2",
            "0.30.10",
            "0.25.0-beta.1",
            "0.20.0",
        ];
        v.sort_by(|a, b| compare_versions(b, a));
        assert_eq!(
            v,
            [
                "0.30.10",
                "0.30.2",
                "0.25.0-beta.1",
                "0.24.0",
                "0.20.0",
                "0.20.0-beta.10",
                "0.20.0-beta.9",
            ]
        );
    }

    #[test]
    fn stable_release_goes_first() {
        let r = |v: &str, stable| LoaderRelease {
            version: LoaderVersion::parse(v).unwrap(),
            stable,
        };
        let got = recommended_first(vec![
            r("0.21.0", false),
            r("0.20.1", true),
            r("0.20.0", false),
        ]);
        let names: Vec<_> = got.iter().map(|r| r.version.as_str()).collect();
        assert_eq!(names, ["0.20.1", "0.21.0", "0.20.0"]);
        assert_eq!(pick(&got, None).unwrap().version.as_str(), "0.20.1");
        let wanted = LoaderVersion::parse("0.20.0").unwrap();
        assert_eq!(pick(&got, Some(&wanted)).unwrap().version, wanted);
        let missing = LoaderVersion::parse("9.9.9").unwrap();
        assert!(pick(&got, Some(&missing)).is_none());
    }
}
