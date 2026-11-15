//! Đổi phiên bản Minecraft hoặc mod loader của server đã có, và cập nhật build Paper, Purpur
//! (spec P3, mục 3.9; rà soát B4). Đi theo luồng chung: hỏi nguồn trước, không có gì mới thì dừng
//! ở đó; có thì tắt server, sao lưu `before_update`, tải bản mới vào thư mục server.
//!
//! Đổi loại loader (ví dụ Fabric sang Forge) không làm trong P3. Mod không tự đổi theo: bản mod cũ
//! không hợp phiên bản mới thì preflight báo trước khi bật.

use std::sync::Arc;

use mh_core::instance::GameSection;
use mh_core::{ErrorCode, LoaderVersion, McVersion, ServerId};

use crate::jobs::{JobContext, JobKind, JobStep};
use crate::update::with_backup;
use crate::{App, AppError, JobId};

/// Số bước khi server đang chạy: hỏi nguồn, tắt, sao lưu, Java, tải, cài loader.
const STEPS: u8 = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionChange {
    /// `None` là giữ bản Minecraft hiện tại (cập nhật build hoặc bản loader).
    pub minecraft: Option<McVersion>,
    /// Chỉ cho mod loader; `None` là bản đề xuất cho phiên bản Minecraft đích.
    pub loader_version: Option<LoaderVersion>,
    /// Người dùng đã đọc hộp thoại: server sẽ bị tắt, có backup, hạ phiên bản có thể hỏng world.
    pub confirm: bool,
}

/// Kết quả Job, nằm trong `result`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VersionChangeResult {
    /// `false` khi đã là bản mới nhất: không tắt server, không tạo backup.
    pub changed: bool,
    pub minecraft: String,
    pub build: Option<u32>,
    pub loader_version: Option<String>,
    pub backup_id: Option<String>,
}

enum Target {
    Server(crate::sources::ResolvedServer),
    Loader(crate::sources::LoaderInstall),
}

impl App {
    /// Job đổi phiên bản. Kiểm tra đầu vào ngay, trước khi tạo Job.
    pub fn change_version(
        self: &Arc<Self>,
        id: ServerId,
        req: VersionChange,
    ) -> Result<JobId, AppError> {
        if !req.confirm {
            return Err(invalid("confirm", "cần xác nhận trước khi đổi phiên bản"));
        }
        let view = self.get(id)?;
        if view.installing.is_some() {
            return Err(AppError::new(ErrorCode::ServerBusy, "server đang được cài"));
        }
        let spec = view
            .spec
            .ok_or_else(|| AppError::new(ErrorCode::ConfigInvalid, "server hỏng cấu hình"))?;
        if req.loader_version.is_some() && !spec.game.loader.is_mod_loader() {
            return Err(invalid("loader_version", "server này không có mod loader"));
        }
        let app = Arc::clone(self);
        Ok(self
            .jobs
            .spawn(JobKind::ChangeVersion, Some(id), move |ctx| async move {
                let result = app.run_change_version(&ctx, id, &req).await?;
                let value = serde_json::to_value(&result).map_err(|err| {
                    AppError::new(ErrorCode::Internal, format!("kết quả đổi phiên bản: {err}"))
                })?;
                ctx.set_result(value);
                Ok(())
            }))
    }

    async fn run_change_version(
        &self,
        ctx: &JobContext,
        id: ServerId,
        req: &VersionChange,
    ) -> Result<VersionChangeResult, AppError> {
        let spec = self.store.load(id)?;
        let loader = spec.game.loader;
        let minecraft = req
            .minecraft
            .clone()
            .unwrap_or_else(|| spec.game.minecraft.clone());
        ctx.step(
            JobStep::ResolveVersion {
                minecraft: minecraft.to_string(),
            },
            1,
            STEPS,
        );
        let target = if loader.is_mod_loader() {
            let wanted = req.loader_version.as_ref();
            Target::Loader(
                ctx.cancellable(self.catalog.plan(loader, &minecraft, wanted))
                    .await?,
            )
        } else {
            Target::Server(
                ctx.cancellable(self.catalog.resolve(loader, &minecraft))
                    .await?,
            )
        };
        if same_as_installed(&spec.game, &target) {
            return Ok(VersionChangeResult::from_game(&spec.game, false, None));
        }
        let (backup, next) = self.prepare_update(ctx, id, 2, STEPS).await?;
        let paths = self.store.paths(id);
        let fetched = match &target {
            Target::Loader(install) => {
                self.fetch_loader(ctx, &paths, loader, install, next, STEPS)
                    .await
            }
            Target::Server(resolved) => {
                self.fetch_vanilla(ctx, &paths, loader, resolved.clone(), next, STEPS)
                    .await
            }
        };
        let game = fetched.map_err(|err| with_backup(err, &backup))?;
        let mut spec = self.store.load(id)?;
        let extra = std::mem::take(&mut spec.game.extra);
        spec.game = GameSection { extra, ..game };
        self.store
            .save(&spec)
            .map_err(|err| with_backup(err.into(), &backup))?;
        self.refresh_preflight(id).await;
        self.notify_changed(id);
        tracing::info!(server = %id, minecraft = %spec.game.minecraft, "đã đổi phiên bản");
        Ok(VersionChangeResult::from_game(
            &spec.game,
            true,
            Some(backup.id),
        ))
    }
}

impl VersionChangeResult {
    fn from_game(game: &GameSection, changed: bool, backup_id: Option<String>) -> Self {
        Self {
            changed,
            minecraft: game.minecraft.to_string(),
            build: game.build,
            loader_version: game.loader_version.as_ref().map(ToString::to_string),
            backup_id,
        }
    }
}

fn same_as_installed(game: &GameSection, target: &Target) -> bool {
    match target {
        Target::Server(resolved) => {
            resolved.minecraft == game.minecraft && resolved.build == game.build
        }
        Target::Loader(install) => {
            install.plan.minecraft == game.minecraft
                && game.loader_version.as_ref() == Some(&install.plan.loader_version)
        }
    }
}

fn invalid(field: &str, detail: &str) -> AppError {
    AppError::new(ErrorCode::InvalidInput, detail)
        .with_params(serde_json::json!({ "field": field }))
}
