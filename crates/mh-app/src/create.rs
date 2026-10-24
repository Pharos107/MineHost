//! Job `create` (spec P1, mục 5.7): kiểm tra ổ đĩa → phiên bản → Java → jar → file cấu hình.

use std::sync::Arc;

use mh_core::instance::GameSection;
use mh_core::{
    ErrorCode, GamePort, InstanceSpec, LoaderKind, McVersion, MemoryLimits, Properties, ServerId,
    ServerName,
};
use time::OffsetDateTime;

use crate::jobs::{JobContext, JobKind, JobStep};
use crate::launcher::write_properties;
use crate::{App, AppError, Installing, JobId, lock};

const STEPS: u8 = 5;

#[derive(Debug, Clone)]
pub struct CreateRequest {
    pub name: ServerName,
    pub loader: LoaderKind,
    pub minecraft: McVersion,
    /// Không có thì dùng đề xuất theo RAM máy.
    pub memory_mb: Option<MemoryLimits>,
    /// Không có thì tự chọn cổng trống từ 25565.
    pub port: Option<GamePort>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Created {
    pub server_id: ServerId,
    pub job_id: JobId,
}

impl App {
    /// Bắt đầu Job cài server; trả ngay ID server và ID Job.
    pub fn create(self: &Arc<Self>, req: CreateRequest) -> Result<Created, AppError> {
        let port = match req.port {
            Some(port) => {
                self.ensure_port_unused(port, None)?;
                port
            }
            None => self.choose_port(None)?,
        };
        let memory = req.memory_mb.unwrap_or_else(|| self.suggested_memory());
        let id = ServerId::generate();
        lock(&self.installing).insert(
            id,
            Installing {
                name: req.name.clone(),
                loader: req.loader,
                minecraft: req.minecraft.clone(),
                port,
                job_id: None,
            },
        );
        let app = Arc::clone(self);
        let job_id = self.jobs.spawn(JobKind::CreateServer, Some(id), move |ctx| async move {
            let result = app.install(&ctx, id, &req, memory, port).await;
            if result.is_err()
                && let Err(e) = app.store.remove_unfinished(id)
            {
                tracing::error!(server = %id, error = %e, "không dọn được thư mục server cài dở");
            }
            lock(&app.installing).remove(&id);
            if result.is_ok() {
                app.supervisor(id);
                tracing::info!(server = %id, "đã tạo server");
            }
            app.notify_changed(id);
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

    async fn install(
        &self,
        ctx: &JobContext,
        id: ServerId,
        req: &CreateRequest,
        memory: MemoryLimits,
        port: GamePort,
    ) -> Result<(), AppError> {
        ctx.step(JobStep::CheckDisk, 1, STEPS);
        self.check_disk()?;

        let minecraft = req.minecraft.to_string();
        ctx.step(
            JobStep::ResolveVersion {
                minecraft: minecraft.clone(),
            },
            2,
            STEPS,
        );
        let resolved = ctx
            .cancellable(self.catalog.resolve(req.loader, &req.minecraft))
            .await?;

        let major = resolved.java_major;
        ctx.step(JobStep::DownloadJava { major }, 3, STEPS);
        {
            let _permit = ctx.download_permit().await?;
            let mut progress = |done: u64, total: Option<u64>| ctx.bytes(done, total);
            self.java.ensure(major, &mut progress, &ctx.cancel).await?;
        }

        ctx.step(JobStep::DownloadServer { minecraft }, 4, STEPS);
        let paths = self.store.create_dir(id)?;
        let game = GameSection::new(req.loader, req.minecraft.clone(), resolved.build, major);
        {
            let _permit = ctx.download_permit().await?;
            let dest = paths.server_dir.join(game.jar.as_str());
            let mut progress = |done: u64, total: Option<u64>| ctx.bytes(done, total);
            self.catalog
                .download(&resolved.jar, &dest, &mut progress, &ctx.cancel)
                .await?;
        }
        ctx.check_cancel()?;

        ctx.step(JobStep::PrepareFiles, 5, STEPS);
        let mut props =
            Properties::parse("# Tạo bởi MineHost; Minecraft sẽ thêm các khóa còn lại.\n");
        props.set("motd", req.name.as_str());
        props.set("server-port", &port.to_string());
        write_properties(&paths.server_dir, &props)?;
        let now = OffsetDateTime::now_utc();
        let now = now.replace_nanosecond(0).unwrap_or(now);
        let spec = InstanceSpec::new(id, req.name.clone(), now, game, memory, port);
        // `instance.toml` ghi sau cùng: có nó nghĩa là server đã cài xong.
        self.store.save(&spec)?;
        Ok(())
    }

    fn check_disk(&self) -> Result<(), AppError> {
        let free = (self.free_space)(self.dirs.root());
        match free {
            Some(free) if free < mh_net::MIN_FREE_BYTES => Err(AppError::new(
                ErrorCode::NotEnoughDisk,
                format!("ổ đĩa chỉ còn {free} byte"),
            )
            .with_params(serde_json::json!({ "available_bytes": free }))),
            _ => Ok(()),
        }
    }
}
