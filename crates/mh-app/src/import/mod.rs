//! Job `import` (spec P2, mục 6.11): chép thư mục server có sẵn vào `servers/<id>/server/`.
//! Bản gốc giữ nguyên; MineHost không sửa gì trong thư mục người dùng chọn.

mod detect;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mh_core::instance::{GameSection, JarName};
use mh_core::{
    ErrorCode, GamePort, InstanceSpec, LoaderKind, McVersion, MemoryLimits, ServerId, ServerName,
};
use time::OffsetDateTime;

use crate::jobs::{JobContext, JobKind, JobStep};
use crate::launcher::{read_properties, write_properties};
use crate::sources::CancellationToken;
use crate::{App, AppError, Created, Installing, lock};

pub use detect::Detected;

const STEPS: u8 = 5;

#[derive(Debug, Clone)]
pub struct ImportRequest {
    /// Đường dẫn tuyệt đối tới thư mục server trên máy này.
    pub path: PathBuf,
    pub name: ServerName,
    /// Người dùng chọn tay khi MineHost không nhận ra; có thì thắng kết quả nhận diện.
    pub loader: Option<LoaderKind>,
    pub minecraft: Option<McVersion>,
}

/// Những gì đã chốt trước khi chạy Job.
struct Plan {
    source: PathBuf,
    loader: LoaderKind,
    minecraft: McVersion,
    jar: JarName,
    port: GamePort,
    memory: MemoryLimits,
}

impl App {
    /// Xem trước thư mục: loại, phiên bản, jar, có `mods/` không. Không chép gì.
    pub fn inspect_import(&self, path: &Path) -> Result<Detected, AppError> {
        let source = self.import_source(path)?;
        Ok(detect::inspect(&source))
    }

    /// Kiểm tra rồi bắt đầu Job nhập; trả ngay ID server và ID Job như `create`.
    pub fn import(self: &Arc<Self>, req: ImportRequest) -> Result<Created, AppError> {
        let plan = self.import_plan(&req)?;
        let id = ServerId::generate();
        lock(&self.installing).insert(
            id,
            Installing {
                name: req.name.clone(),
                loader: plan.loader,
                minecraft: plan.minecraft.clone(),
                port: plan.port,
                job_id: None,
            },
        );
        let app = Arc::clone(self);
        let name = req.name;
        let job_id = self
            .jobs
            .spawn(JobKind::ImportServer, Some(id), move |ctx| async move {
                let result = app.run_import(&ctx, id, name, &plan).await;
                app.finish_install(id, result.is_ok(), "đã nhập server");
                result
            });
        if let Some(entry) = lock(&self.installing).get_mut(&id) {
            entry.job_id = Some(job_id);
        }
        self.notify_changed(id);
        Ok(Created {
            server_id: id,
            job_id,
        })
    }

    fn import_plan(&self, req: &ImportRequest) -> Result<Plan, AppError> {
        let source = self.import_source(&req.path)?;
        let found = detect::inspect(&source);
        if found.has_mods {
            return Err(AppError::new(
                ErrorCode::ModsNotSupported,
                "thư mục có mods/, chưa hỗ trợ ở P2",
            ));
        }
        let missing = |field: &str| {
            AppError::new(ErrorCode::InvalidInput, format!("không nhận ra {field}"))
                .with_params(serde_json::json!({ "field": field }))
        };
        let loader = req
            .loader
            .or(found.loader)
            .ok_or_else(|| missing("loader"))?;
        let minecraft = req
            .minecraft
            .clone()
            .or(found.minecraft)
            .ok_or_else(|| missing("minecraft"))?;
        let jar = found.jar.ok_or_else(|| missing("jar"))?;
        Ok(Plan {
            port: self.import_port(&source)?,
            memory: self.suggested_memory(),
            source,
            loader,
            minecraft,
            jar,
        })
    }

    /// Thư mục phải tồn tại, là đường dẫn tuyệt đối, và không lồng với thư mục dữ liệu của
    /// MineHost (chép `data/` vào chính nó sẽ không bao giờ xong).
    fn import_source(&self, path: &Path) -> Result<PathBuf, AppError> {
        let invalid = || {
            AppError::new(ErrorCode::InvalidInput, "thư mục không dùng được")
                .with_params(serde_json::json!({ "field": "path" }))
        };
        if !path.is_absolute() {
            return Err(invalid());
        }
        let source = std::fs::canonicalize(path).map_err(|_| invalid())?;
        if !source.is_dir() {
            return Err(invalid());
        }
        let root = std::fs::canonicalize(self.dirs.root())
            .unwrap_or_else(|_| self.dirs.root().to_path_buf());
        if source.starts_with(&root) || root.starts_with(&source) {
            return Err(invalid());
        }
        Ok(source)
    }

    /// Giữ cổng trong `server.properties` nếu còn trống, không thì chọn cổng trống như khi tạo mới.
    fn import_port(&self, source: &Path) -> Result<GamePort, AppError> {
        let current = read_properties(source)
            .ok()
            .and_then(|props| props.get("server-port").and_then(|p| p.trim().parse().ok()))
            .and_then(|port: u16| GamePort::new(port).ok());
        match current {
            Some(port)
                if self.ensure_port_unused(port, None).is_ok() && (self.port_free)(port.get()) =>
            {
                Ok(port)
            }
            _ => self.choose_port(None),
        }
    }

    async fn run_import(
        &self,
        ctx: &JobContext,
        id: ServerId,
        name: ServerName,
        plan: &Plan,
    ) -> Result<(), AppError> {
        ctx.step(JobStep::CheckDisk, 1, STEPS);
        let size = mh_platform::dir_size(&plan.source);
        self.check_disk_for(size)?;

        let minecraft = plan.minecraft.to_string();
        ctx.step(JobStep::ResolveVersion { minecraft }, 2, STEPS);
        // Jar của Mojang cho biết bản Java cần dùng, kể cả khi server là Paper hay Purpur.
        let resolved = ctx
            .cancellable(self.catalog.resolve(LoaderKind::Vanilla, &plan.minecraft))
            .await?;
        let major = resolved.java_major;

        ctx.step(JobStep::DownloadJava { major }, 3, STEPS);
        {
            let _permit = ctx.download_permit().await?;
            let mut progress = |done: u64, total: Option<u64>| ctx.bytes(done, total);
            self.java.ensure(major, &mut progress, &ctx.cancel).await?;
        }

        ctx.step(JobStep::CopyFiles, 4, STEPS);
        let paths = self.store.create_dir(id)?;
        self.copy_server(ctx, &plan.source, &paths.server_dir, size)
            .await?;

        ctx.step(JobStep::PrepareFiles, 5, STEPS);
        let mut props = read_properties(&paths.server_dir)?;
        props.set("server-port", &plan.port.to_string());
        write_properties(&paths.server_dir, &props)?;
        let mut game = GameSection::new(plan.loader, plan.minecraft.clone(), None, major);
        game.jar = plan.jar.clone();
        let now = OffsetDateTime::now_utc();
        let now = now.replace_nanosecond(0).unwrap_or(now);
        // EULA không tự đánh dấu dù thư mục đã có `eula=true`: người dùng xác nhận lại trong MineHost.
        let spec = InstanceSpec::new(id, name, now, game, plan.memory, plan.port);
        self.store.save(&spec)?;
        Ok(())
    }

    async fn copy_server(
        &self,
        ctx: &JobContext,
        source: &Path,
        dest: &Path,
        total: u64,
    ) -> Result<(), AppError> {
        let (source, dest) = (source.to_path_buf(), dest.to_path_buf());
        let cancel = ctx.cancel.clone();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<u64>();
        let task = tokio::task::spawn_blocking(move || {
            let mut copied = 0u64;
            copy_tree(&source, &dest, &cancel, &mut |n| {
                copied += n;
                let _ = tx.send(copied);
            })
        });
        let total = (total > 0).then_some(total);
        while let Some(done) = rx.recv().await {
            ctx.bytes(done, total);
        }
        match task.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(CopyError::Cancelled)) => Err(AppError::new(ErrorCode::JobCancelled, "đã hủy")),
            Ok(Err(CopyError::Io(path, err))) => Err(AppError::new(
                ErrorCode::Internal,
                format!("không chép được {}: {err}", path.display()),
            )),
            Err(err) => Err(AppError::new(
                ErrorCode::Internal,
                format!("luồng chép dừng: {err}"),
            )),
        }
    }
}

enum CopyError {
    Cancelled,
    Io(PathBuf, io::Error),
}

/// Chép đệ quy. Symlink bị bỏ qua, để không đi theo liên kết ra ngoài thư mục người dùng chọn.
fn copy_tree(
    source: &Path,
    dest: &Path,
    cancel: &CancellationToken,
    progress: &mut dyn FnMut(u64),
) -> Result<(), CopyError> {
    std::fs::create_dir_all(dest).map_err(|e| CopyError::Io(dest.to_path_buf(), e))?;
    let entries = std::fs::read_dir(source).map_err(|e| CopyError::Io(source.to_path_buf(), e))?;
    for entry in entries {
        if cancel.is_cancelled() {
            return Err(CopyError::Cancelled);
        }
        let entry = entry.map_err(|e| CopyError::Io(source.to_path_buf(), e))?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        let kind = entry
            .file_type()
            .map_err(|e| CopyError::Io(from.clone(), e))?;
        if kind.is_symlink() {
            tracing::warn!(path = %from.display(), "bỏ qua symlink khi nhập server");
        } else if kind.is_dir() {
            copy_tree(&from, &to, cancel, progress)?;
        } else if kind.is_file() {
            let n = std::fs::copy(&from, &to).map_err(|e| CopyError::Io(from.clone(), e))?;
            progress(n);
        }
    }
    Ok(())
}
