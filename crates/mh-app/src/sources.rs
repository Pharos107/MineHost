//! Nguồn phiên bản/jar server và nguồn Java, tách thành trait để test chạy không cần mạng.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use mh_core::{ErrorCode, LoaderKind, LoaderVersion, McVersion};
use mh_java::JavaManager;
use mh_net::{Download, HttpClient};
use mh_providers::{Fabric, Forge, Mojang, NeoForge, Paper, Purpur, Quilt, Release};
use time::OffsetDateTime;

use crate::AppError;

// Kiểu cần để tự cài `ServerCatalog`/`JavaSource` (ví dụ trong test) mà không phụ thuộc thẳng
// các crate tầng dưới.
pub use mh_java::JavaRuntime;
pub use mh_net::{CancellationToken, Checksum};
pub use mh_providers::{InstallPlan, InstallerRun, LoaderRelease, PlannedFile, ServerJar};

pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type ProgressFn<'a> = &'a mut (dyn FnMut(u64, Option<u64>) + Send);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogVersion {
    pub id: McVersion,
    pub release_time: OffsetDateTime,
}

/// Những gì cần để cài một phiên bản cụ thể.
/// Kế hoạch cài mod loader kèm bản Java mà Minecraft đó cần (spec P3, mục 3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderInstall {
    pub plan: InstallPlan,
    pub java_major: u16,
}

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

    /// Bản mod loader cho một phiên bản Minecraft, bản đề xuất đứng đầu. Catalog không có mod
    /// loader (ví dụ trong test của P2) dùng bản mặc định này.
    fn loader_versions<'a>(
        &'a self,
        loader: LoaderKind,
        _minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<LoaderRelease>, AppError>> {
        Box::pin(std::future::ready(Err(unsupported(loader))))
    }

    /// Kế hoạch cài mod loader; `wanted` không có thì dùng bản đề xuất.
    fn plan<'a>(
        &'a self,
        loader: LoaderKind,
        _minecraft: &'a McVersion,
        _wanted: Option<&'a LoaderVersion>,
    ) -> BoxFut<'a, Result<LoaderInstall, AppError>> {
        Box::pin(std::future::ready(Err(unsupported(loader))))
    }

    /// Tải một file trong kế hoạch cài, chỉ từ host mà kế hoạch cho phép.
    fn download_file<'a>(
        &'a self,
        _file: &'a PlannedFile,
        _dest: &'a Path,
        _progress: ProgressFn<'a>,
        _cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(std::future::ready(Err(AppError::new(
            ErrorCode::Internal,
            "catalog này không tải file của mod loader",
        ))))
    }
}

pub trait JavaSource: Send + Sync + 'static {
    /// Kiểm tra nhanh xem bản Java đã có sẵn trên đĩa chưa (mặc định true cho nguồn giả lập).
    fn is_installed(&self, _major: u16) -> bool {
        true
    }

    fn ensure<'a>(
        &'a self,
        major: u16,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<JavaRuntime, AppError>>;
}

/// Tham số chạy sau `-jar` hoặc `@args_file` theo loại server.
pub fn launch_args(loader: LoaderKind) -> Vec<String> {
    match loader {
        // Purpur là bản sửa của Paper nên nhận cùng tham số.
        LoaderKind::Paper | LoaderKind::Purpur => Paper::recipe().args,
        // Jar của Mojang nhận `nogui` không có gạch ngang, đúng từ các bản rất cũ. Bốn mod
        // loader chuyển tham số cho lớp main của Minecraft nên nhận cùng kiểu.
        LoaderKind::Vanilla
        | LoaderKind::Fabric
        | LoaderKind::Quilt
        | LoaderKind::Forge
        | LoaderKind::NeoForge => vec!["nogui".to_owned()],
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

/// Mojang cho biết bản Java cần dùng (và jar Vanilla); Paper, Purpur cho jar của chúng; bốn mod
/// loader cho kế hoạch cài.
#[derive(Debug)]
pub struct OnlineCatalog {
    http: Arc<HttpClient>,
    mojang: Mojang,
    paper: Paper,
    purpur: Purpur,
    fabric: Fabric,
    quilt: Quilt,
    forge: Forge,
    neoforge: NeoForge,
}

impl OnlineCatalog {
    pub fn new(http: Arc<HttpClient>) -> Self {
        Self {
            mojang: Mojang::new(Arc::clone(&http)),
            paper: Paper::new(Arc::clone(&http)),
            purpur: Purpur::new(Arc::clone(&http)),
            fabric: Fabric::new(Arc::clone(&http)),
            quilt: Quilt::new(Arc::clone(&http)),
            forge: Forge::new(Arc::clone(&http)),
            neoforge: NeoForge::new(Arc::clone(&http)),
            http,
        }
    }

    async fn loader_releases(
        &self,
        loader: LoaderKind,
        mc: &McVersion,
    ) -> Result<Vec<LoaderRelease>, AppError> {
        Ok(match loader {
            LoaderKind::Fabric => self.fabric.loader_versions(mc).await?,
            LoaderKind::Quilt => self.quilt.loader_versions(mc).await?,
            LoaderKind::Forge => self.forge.loader_versions(mc).await?,
            LoaderKind::NeoForge => self.neoforge.loader_versions(mc).await?,
            _ => return Err(unsupported(loader)),
        })
    }

    async fn loader_plan(
        &self,
        loader: LoaderKind,
        mc: &McVersion,
        wanted: Option<&LoaderVersion>,
    ) -> Result<LoaderInstall, AppError> {
        let detail = self.mojang.version_detail(mc);
        let plan = async {
            Ok::<_, AppError>(match loader {
                LoaderKind::Fabric => self.fabric.plan(mc, wanted).await?,
                LoaderKind::Quilt => self.quilt.plan(mc, wanted).await?,
                LoaderKind::Forge => self.forge.plan(mc, wanted).await?,
                LoaderKind::NeoForge => self.neoforge.plan(mc, wanted).await?,
                _ => return Err(unsupported(loader)),
            })
        };
        let (detail, plan) = tokio::join!(detail, plan);
        Ok(LoaderInstall {
            plan: plan?,
            java_major: detail?.java_major,
        })
    }
}

impl OnlineCatalog {
    async fn releases_for(&self, loader: LoaderKind) -> Result<Vec<Release>, AppError> {
        let releases = self.mojang.releases().await?;
        Ok(match loader {
            LoaderKind::Paper => self.paper.versions(&releases).await?,
            LoaderKind::Purpur => self.purpur.versions(&releases).await?,
            LoaderKind::Vanilla => releases,
            LoaderKind::Fabric => self.fabric.versions(&releases).await?,
            LoaderKind::Quilt => self.quilt.versions(&releases).await?,
            LoaderKind::Forge => self.forge.versions(&releases).await?,
            LoaderKind::NeoForge => self.neoforge.versions(&releases).await?,
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
                allowed_hosts: None,
            };
            self.http.download(req, progress, cancel).await?;
            Ok(())
        })
    }

    fn loader_versions<'a>(
        &'a self,
        loader: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<LoaderRelease>, AppError>> {
        Box::pin(self.loader_releases(loader, minecraft))
    }

    fn plan<'a>(
        &'a self,
        loader: LoaderKind,
        minecraft: &'a McVersion,
        wanted: Option<&'a LoaderVersion>,
    ) -> BoxFut<'a, Result<LoaderInstall, AppError>> {
        Box::pin(self.loader_plan(loader, minecraft, wanted))
    }

    fn download_file<'a>(
        &'a self,
        file: &'a PlannedFile,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            let req = Download {
                url: &file.url,
                checksum: &file.checksum,
                dest,
                size: file.size,
                allowed_hosts: Some(&file.hosts),
            };
            self.http.download(req, progress, cancel).await?;
            Ok(())
        })
    }
}

impl JavaSource for JavaManager {
    fn is_installed(&self, major: u16) -> bool {
        JavaManager::is_installed(self, major)
    }

    fn ensure<'a>(
        &'a self,
        major: u16,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<JavaRuntime, AppError>> {
        Box::pin(async move { Ok(JavaManager::ensure(self, major, progress, cancel).await?) })
    }
}
