//! Job `create` (spec P1, mục 5.7): kiểm tra ổ đĩa → phiên bản → Java → jar → file cấu hình.

use std::sync::Arc;

use std::path::Path;

use mh_core::instance::{GameSection, ModpackSection};
use mh_core::{
    ErrorCode, GamePort, InstanceSpec, LoaderKind, LoaderVersion, McVersion, MemoryLimits,
    Properties, ServerId, ServerName,
};
use mh_store::ServerPaths;
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
    /// Bản mod loader (spec P3, mục 4.1); không có thì dùng bản đề xuất. Bỏ qua với Paper,
    /// Purpur, Vanilla.
    pub loader_version: Option<LoaderVersion>,
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
        let port = self.resolve_new_port(req.port)?;
        let memory = req.memory_mb.unwrap_or_else(|| self.suggested_memory());
        let id = ServerId::generate();
        lock(&self.installing).insert(
            id,
            Installing {
                name: req.name.clone(),
                loader: Some(req.loader),
                minecraft: Some(req.minecraft.clone()),
                port,
                job_id: None,
            },
        );
        let app = Arc::clone(self);
        let job_id = self
            .jobs
            .spawn(JobKind::CreateServer, Some(id), move |ctx| async move {
                let result = app.install(&ctx, id, &req, memory, port).await;
                app.finish_install(id, result.is_ok(), "đã tạo server");
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

    pub(crate) fn resolve_new_port(
        &self,
        requested: Option<GamePort>,
    ) -> Result<GamePort, AppError> {
        match requested {
            Some(port) => {
                self.ensure_port_unused(port, None)?;
                Ok(port)
            }
            None => self.choose_port(None),
        }
    }

    async fn install(
        &self,
        ctx: &JobContext,
        id: ServerId,
        req: &CreateRequest,
        memory: MemoryLimits,
        port: GamePort,
    ) -> Result<(), AppError> {
        if req.loader.is_mod_loader() {
            return self.install_with_loader(ctx, id, req, memory, port).await;
        }
        ctx.step(JobStep::CheckDisk, 1, STEPS);
        self.check_disk()?;

        let (paths, game) = self
            .setup_vanilla_server(ctx, id, req.loader, &req.minecraft, 2, STEPS)
            .await?;

        ctx.step(JobStep::PrepareFiles, 5, STEPS);
        self.write_new_server(&paths.server_dir, id, req, game, memory, port)
    }

    pub(crate) async fn setup_vanilla_server(
        &self,
        ctx: &JobContext,
        id: ServerId,
        loader: LoaderKind,
        mc: &McVersion,
        step_base: u8,
        step_count: u8,
    ) -> Result<(ServerPaths, GameSection), AppError> {
        let minecraft = mc.to_string();
        ctx.step(
            JobStep::ResolveVersion {
                minecraft: minecraft.clone(),
            },
            step_base,
            step_count,
        );
        let resolved = ctx.cancellable(self.catalog.resolve(loader, mc)).await?;

        let major = resolved.java_major;
        ctx.step(JobStep::DownloadJava { major }, step_base + 1, step_count);
        {
            let _permit = ctx.download_permit().await?;
            let mut progress = |done: u64, total: Option<u64>| ctx.bytes(done, total);
            self.java.ensure(major, &mut progress, &ctx.cancel).await?;
        }

        ctx.step(
            JobStep::DownloadServer { minecraft },
            step_base + 2,
            step_count,
        );
        let paths = self.store.create_dir(id)?;
        let game = GameSection::new(loader, mc.clone(), resolved.build, major);
        {
            let _permit = ctx.download_permit().await?;
            let dest = paths.server_dir.join(game.jar.as_str());
            let mut progress = |done: u64, total: Option<u64>| ctx.bytes(done, total);
            self.catalog
                .download(&resolved.jar, &dest, &mut progress, &ctx.cancel)
                .await?;
        }
        ctx.check_cancel()?;
        Ok((paths, game))
    }

    /// Bước cuối của mọi kiểu tạo server: `server.properties`, rồi `instance.toml`.
    pub(crate) fn write_new_server(
        &self,
        server_dir: &Path,
        id: ServerId,
        req: &CreateRequest,
        game: GameSection,
        memory: MemoryLimits,
        port: GamePort,
    ) -> Result<(), AppError> {
        self.write_server_spec(server_dir, id, req.name.clone(), game, memory, port, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn write_server_spec(
        &self,
        server_dir: &Path,
        id: ServerId,
        name: ServerName,
        game: GameSection,
        memory: MemoryLimits,
        port: GamePort,
        modpack: Option<ModpackSection>,
    ) -> Result<(), AppError> {
        let mut props =
            Properties::parse("# Tạo bởi MineHost; Minecraft sẽ thêm các khóa còn lại.\n");
        props.set("motd", name.as_str());
        props.set("server-port", &port.to_string());
        write_properties(server_dir, &props)?;
        let now = OffsetDateTime::now_utc();
        let now = now.replace_nanosecond(0).unwrap_or(now);
        let mut spec = InstanceSpec::new(id, name, now, game, memory, port);
        spec.modpack = modpack;
        // `instance.toml` ghi sau cùng: có nó nghĩa là server đã cài xong.
        self.store.save(&spec)?;
        Ok(())
    }

    /// Job tạo hoặc nhập xong: lỗi thì xóa thư mục làm dở; xong thì tạo supervisor.
    pub(crate) fn finish_install(&self, id: ServerId, ok: bool, done: &str) {
        if !ok && let Err(e) = self.store.remove_unfinished(id) {
            tracing::error!(server = %id, error = %e, "không dọn được thư mục server cài dở");
        }
        lock(&self.installing).remove(&id);
        if ok {
            self.supervisor(id);
            tracing::info!(server = %id, "{done}");
        }
        self.notify_changed(id);
    }

    pub(crate) fn check_disk(&self) -> Result<(), AppError> {
        self.check_disk_for(0)
    }

    /// Ổ dữ liệu phải còn ít nhất `needed` byte cộng phần chừa lại tối thiểu.
    pub(crate) fn check_disk_for(&self, needed: u64) -> Result<(), AppError> {
        let free = (self.free_space)(self.dirs.root());
        match free {
            Some(free) if free < mh_net::MIN_FREE_BYTES.saturating_add(needed) => {
                Err(AppError::new(
                    ErrorCode::NotEnoughDisk,
                    format!("ổ đĩa chỉ còn {free} byte"),
                )
                .with_params(serde_json::json!({ "available_bytes": free })))
            }
            _ => Ok(()),
        }
    }
}
