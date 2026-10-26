//! Catalog Paper và Java giả cho test giao diện. Không gọi mạng.
//!
//! Chỉ được biên dịch khi bật feature `e2e` của `mh-app` (binary `minehost` bật feature cùng tên).
//! Controller dùng module này khi có [`JAVA_BIN_ENV`] trỏ tới `fake-mc`. Không có biến đó thì
//! `minehost` vẫn dựng `App::new` với Mojang, Paper và Adoptium.
//!
//! Trên Linux:
//!
//! ```text
//! cargo build -p mh-runtime --bin fake-mc
//! cargo build -p minehost --features e2e
//! MINEHOST_E2E_JAVA="$PWD/target/debug/fake-mc" \
//!   ./target/debug/minehost --data-dir /tmp/mh-e2e --no-browser --no-tray
//! ```
//!
//! `cargo build -p minehost --features e2e` cũng tạo `target/debug/e2e-fake-mc` từ cùng mã nguồn,
//! để test Rust lấy đường dẫn qua `CARGO_BIN_EXE`. Playwright nên dùng `fake-mc` của `mh-runtime`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use mh_core::{ErrorCode, LoaderKind, McVersion};
use mh_supervisor::SupervisorConfig;
use time::OffsetDateTime;
use time::macros::datetime;

use crate::sources::{
    BoxFut, CancellationToken, CatalogVersion, Checksum, JavaRuntime, JavaSource, ProgressFn,
    ResolvedServer, ServerCatalog, ServerJar,
};
use crate::{AppDeps, AppError};

/// Tên biến môi trường mà binary `minehost` đọc. Giá trị là đường dẫn tới `fake-mc`.
/// Binary đổi đường dẫn tương đối thành tuyệt đối theo cwd của controller trước khi gọi [`deps`].
pub const JAVA_BIN_ENV: &str = "MINEHOST_E2E_JAVA";

const JAVA_MAJOR: u16 = 21;
const JAVA_VERSION: &str = "21.0.5";
/// Không phải build Paper thật: jar không được tải, số này chỉ ghi vào `instance.toml`.
const FAKE_BUILD: u32 = 1;
const FAKE_JAR: &[u8] = b"minehost-e2e-fake-jar\n";
const FAKE_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";

struct PaperRelease {
    id: &'static str,
    released: OffsetDateTime,
}

/// Mới nhất trước, cùng quy ước với `ServerCatalog::versions`.
const KNOWN: &[PaperRelease] = &[
    PaperRelease {
        id: "1.21.4",
        released: datetime!(2024-12-03 0:00 UTC),
    },
    PaperRelease {
        id: "1.20.4",
        released: datetime!(2023-12-07 0:00 UTC),
    },
];

/// Catalog giả, Java giả, còn cổng / ổ đĩa / RAM giống `App::new`.
pub fn deps(java_bin: PathBuf) -> AppDeps {
    let (port_free, free_space, total_memory_mb) = crate::host_deps_parts();
    AppDeps {
        catalog: Arc::new(E2eCatalog),
        java: Arc::new(E2eJava { java_bin }),
        port_free,
        free_space,
        total_memory_mb,
        supervisor: SupervisorConfig::default(),
    }
}

#[derive(Debug)]
struct E2eCatalog;

impl ServerCatalog for E2eCatalog {
    fn versions(&self, loader: LoaderKind) -> BoxFut<'_, Result<Vec<CatalogVersion>, AppError>> {
        Box::pin(std::future::ready(versions_of(loader)))
    }

    fn resolve<'a>(
        &'a self,
        loader: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<ResolvedServer, AppError>> {
        Box::pin(std::future::ready(resolve_of(loader, minecraft)))
    }

    fn download<'a>(
        &'a self,
        _jar: &'a ServerJar,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(std::future::ready(write_fake_jar(dest, progress, cancel)))
    }
}

fn versions_of(loader: LoaderKind) -> Result<Vec<CatalogVersion>, AppError> {
    require_paper(loader)?;
    let mut out = Vec::with_capacity(KNOWN.len());
    for item in KNOWN {
        out.push(CatalogVersion {
            id: McVersion::parse(item.id)?,
            release_time: item.released,
        });
    }
    Ok(out)
}

fn resolve_of(loader: LoaderKind, minecraft: &McVersion) -> Result<ResolvedServer, AppError> {
    require_paper(loader)?;
    let Some(known) = KNOWN.iter().find(|item| item.id == minecraft.as_str()) else {
        return Err(AppError::new(
            ErrorCode::VersionNotFound,
            format!("không có Paper {minecraft} trong catalog e2e"),
        ));
    };
    Ok(ResolvedServer {
        minecraft: minecraft.clone(),
        build: Some(FAKE_BUILD),
        java_major: JAVA_MAJOR,
        jar: fake_jar(known.id)?,
    })
}

fn require_paper(loader: LoaderKind) -> Result<(), AppError> {
    if loader == LoaderKind::Paper {
        Ok(())
    } else {
        Err(crate::sources::unsupported(loader))
    }
}

fn fake_jar(version: &str) -> Result<ServerJar, AppError> {
    // Hash không được kiểm tra: `download` ghi file tại chỗ, không đi qua HTTP.
    let checksum = Checksum::sha256(FAKE_SHA256).ok_or_else(|| {
        AppError::new(
            ErrorCode::Internal,
            "checksum giả của chế độ e2e không hợp lệ",
        )
    })?;
    Ok(ServerJar {
        url: format!("e2e://paper/{version}/server.jar"),
        checksum,
        size: FAKE_JAR.len() as u64,
    })
}

fn write_fake_jar(
    dest: &Path,
    progress: ProgressFn<'_>,
    cancel: &CancellationToken,
) -> Result<(), AppError> {
    if cancel.is_cancelled() {
        return Err(AppError::cancelled());
    }
    let size = FAKE_JAR.len() as u64;
    progress(size, Some(size));
    mh_platform::fs::write_atomic(dest, FAKE_JAR)?;
    Ok(())
}

#[derive(Debug)]
struct E2eJava {
    java_bin: PathBuf,
}

impl JavaSource for E2eJava {
    fn ensure<'a>(
        &'a self,
        major: u16,
        _progress: ProgressFn<'a>,
        _cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<JavaRuntime, AppError>> {
        Box::pin(std::future::ready(Ok(self.runtime(major))))
    }
}

impl E2eJava {
    fn runtime(&self, major: u16) -> JavaRuntime {
        let java_bin = self.java_bin.clone();
        // `home` không được launcher dùng; điền thư mục chứa binary cho đủ `JavaRuntime`.
        let home = java_bin
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        JavaRuntime {
            major,
            home,
            java_bin,
            version: JAVA_VERSION.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use mh_core::{ErrorCode, LoaderKind, McVersion};

    use super::{E2eCatalog, E2eJava, FAKE_BUILD, FAKE_JAR, JAVA_MAJOR, JAVA_VERSION};
    use crate::sources::{CancellationToken, JavaSource, ServerCatalog};

    #[tokio::test]
    async fn paper_catalog_is_offline_and_fixed() {
        let catalog = E2eCatalog;
        let versions = catalog.versions(LoaderKind::Paper).await.unwrap();
        let ids: Vec<_> = versions.iter().map(|v| v.id.as_str().to_owned()).collect();
        assert_eq!(ids, ["1.21.4", "1.20.4"]);

        let old = McVersion::parse("1.20.4").unwrap();
        let resolved = catalog.resolve(LoaderKind::Paper, &old).await.unwrap();
        assert_eq!(resolved.java_major, JAVA_MAJOR);
        assert_eq!(resolved.build, Some(FAKE_BUILD));

        let missing = McVersion::parse("1.16.5").unwrap();
        let err = catalog
            .resolve(LoaderKind::Paper, &missing)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::VersionNotFound);

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("server.jar");
        let cancel = CancellationToken::new();
        catalog
            .download(&resolved.jar, &dest, &mut |_, _| {}, &cancel)
            .await
            .unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), FAKE_JAR);

        cancel.cancel();
        let blocked = dir.path().join("other.jar");
        let err = catalog
            .download(&resolved.jar, &blocked, &mut |_, _| {}, &cancel)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::JobCancelled);
        assert!(!blocked.exists());
    }

    #[tokio::test]
    async fn java_points_at_the_given_binary() {
        let java_bin = PathBuf::from("java").join("fake-mc");
        let java = E2eJava {
            java_bin: java_bin.clone(),
        };
        let cancel = CancellationToken::new();
        let runtime = java.ensure(17, &mut |_, _| {}, &cancel).await.unwrap();
        assert_eq!(runtime.major, 17);
        assert_eq!(runtime.version, JAVA_VERSION);
        assert_eq!(runtime.java_bin, java_bin);
        assert_eq!(runtime.home, PathBuf::from("java"));
    }
}
