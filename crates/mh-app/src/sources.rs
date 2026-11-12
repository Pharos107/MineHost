//! Nguồn phiên bản/jar server và nguồn Java, tách thành trait để test chạy không cần mạng.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use mh_core::{ErrorCode, LoaderKind, McVersion};
use mh_java::JavaManager;
use mh_net::{Download, HttpClient};
use mh_providers::{Mojang, Paper, Purpur, Release};
use time::OffsetDateTime;

use crate::AppError;

// Kiểu cần để tự cài `ServerCatalog`/`JavaSource` (ví dụ trong test) mà không phụ thuộc thẳng
// các crate tầng dưới.
pub use mh_java::JavaRuntime;
pub use mh_net::{CancellationToken, Checksum};
pub use mh_providers::ServerJar;

pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type ProgressFn<'a> = &'a mut (dyn FnMut(u64, Option<u64>) + Send);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogVersion {
    pub id: McVersion,
    pub release_time: OffsetDateTime,
}

/// Những gì cần để cài một phiên bản cụ thể.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedServer {
    pub minecraft: McVersion,
    pub build: Option<u32>,
    pub java_major: u16,
    pub jar: ServerJar,
}

pub trait ServerCatalog: Send + Sync + 'static {
    /// Phiên bản cài được, mới nhất trước.
    fn versions(&self, loader: LoaderKind) -> BoxFut<'_, Result<Vec<CatalogVersion>, AppError>>;

    fn resolve<'a>(
        &'a self,
        loader: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<ResolvedServer, AppError>>;

    fn download<'a>(
        &'a self,
        jar: &'a ServerJar,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>>;
}

pub trait JavaSource: Send + Sync + 'static {
    fn ensure<'a>(
        &'a self,
        major: u16,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<JavaRuntime, AppError>>;
}

/// Tham số chạy sau `-jar` theo loại server.
pub fn launch_args(loader: LoaderKind) -> Vec<String> {
    match loader {
        // Purpur là bản sửa của Paper nên nhận cùng tham số.
        LoaderKind::Paper | LoaderKind::Purpur => Paper::recipe().args,
        // Jar của Mojang nhận `nogui` không có gạch ngang, đúng từ các bản rất cũ.
        LoaderKind::Vanilla => vec!["nogui".to_owned()],
        // `LoaderKind` là `non_exhaustive`; loại mới phải tự khai báo cách chạy ở đây.
        _ => Vec::new(),
    }
}

pub(crate) fn unsupported(loader: LoaderKind) -> AppError {
    AppError::new(
        ErrorCode::InvalidInput,
        format!("loại server {loader} chưa hỗ trợ"),
    )
}

/// Mojang cho biết bản Java cần dùng (và jar Vanilla); Paper, Purpur cho jar của chúng.
#[derive(Debug)]
pub struct OnlineCatalog {
    http: Arc<HttpClient>,
    mojang: Mojang,
    paper: Paper,
    purpur: Purpur,
}

impl OnlineCatalog {
    pub fn new(http: Arc<HttpClient>) -> Self {
        Self {
            mojang: Mojang::new(Arc::clone(&http)),
            paper: Paper::new(Arc::clone(&http)),
            purpur: Purpur::new(Arc::clone(&http)),
            http,
        }
    }
}

impl OnlineCatalog {
    async fn releases_for(&self, loader: LoaderKind) -> Result<Vec<Release>, AppError> {
        let releases = self.mojang.releases().await?;
        Ok(match loader {
            LoaderKind::Paper => self.paper.versions(&releases).await?,
            LoaderKind::Purpur => self.purpur.versions(&releases).await?,
            LoaderKind::Vanilla => releases,
            _ => return Err(unsupported(loader)),
        })
    }

    /// Bản Java lấy từ Mojang, jar từ nguồn của loại server. Hai request chạy song song.
    async fn resolve_jar(
        &self,
        loader: LoaderKind,
        minecraft: &McVersion,
    ) -> Result<ResolvedServer, AppError> {
        let detail = self.mojang.version_detail(minecraft);
        let (java_major, build, jar) = match loader {
            LoaderKind::Paper => {
                let (detail, build) =
                    tokio::join!(detail, self.paper.latest_stable_build(minecraft));
                let build = build?;
                (detail?.java_major, Some(build.build), build.jar)
            }
            LoaderKind::Purpur => {
                let (detail, build) = tokio::join!(detail, self.purpur.latest_build(minecraft));
                let build = build?;
                (detail?.java_major, Some(build.build), build.jar)
            }
            LoaderKind::Vanilla => {
                let detail = detail.await?;
                // Vài bản rất cũ không có jar server.
                let jar = detail.server.ok_or_else(|| {
                    AppError::new(
                        ErrorCode::VersionNotFound,
                        format!("{minecraft} không có jar server"),
                    )
                })?;
                (detail.java_major, None, jar)
            }
            _ => return Err(unsupported(loader)),
        };
        Ok(ResolvedServer {
            minecraft: minecraft.clone(),
            build,
            java_major,
            jar,
        })
    }
}

impl ServerCatalog for OnlineCatalog {
    fn versions(&self, loader: LoaderKind) -> BoxFut<'_, Result<Vec<CatalogVersion>, AppError>> {
        Box::pin(async move {
            Ok(self
                .releases_for(loader)
                .await?
                .into_iter()
                .map(|r| CatalogVersion {
                    id: r.id,
                    release_time: r.release_time,
                })
                .collect())
        })
    }

    fn resolve<'a>(
        &'a self,
        loader: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<ResolvedServer, AppError>> {
        Box::pin(self.resolve_jar(loader, minecraft))
    }

    fn download<'a>(
        &'a self,
        jar: &'a ServerJar,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            let req = Download {
                url: &jar.url,
                checksum: &jar.checksum,
                dest,
                // Purpur không ghi kích thước (0): để `Content-Length` cho biết.
                size: (jar.size > 0).then_some(jar.size),
            };
            self.http.download(req, progress, cancel).await?;
            Ok(())
        })
    }
}

impl JavaSource for JavaManager {
    fn ensure<'a>(
        &'a self,
        major: u16,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<JavaRuntime, AppError>> {
        Box::pin(async move { Ok(JavaManager::ensure(self, major, progress, cancel).await?) })
    }
}
