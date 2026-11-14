//! Job tạo server có mod loader (spec P3, mục 3.1; ADR 0006): tải installer, chạy installer bằng
//! Java của server, rồi kiểm tra trên đĩa rằng file chạy server thật sự đã có.

use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;

use mh_core::instance::{GameSection, LaunchMode, LaunchSection};
use mh_core::{
    ArgsOs, ErrorCode, GamePort, Launch, LoaderKind, LoaderVersion, McVersion, MemoryLimits,
    ServerId,
};
use mh_platform::SafeRoot;
use mh_providers::loader::INSTALL_DIR;
use mh_runtime::{Completion, ENCODING_FLAGS, ProcessSpec, ResourceLimits};
use mh_store::ServerPaths;

use crate::jobs::{JobContext, JobStep};
use crate::sources::{InstallerRun, LoaderInstall};
use crate::{App, AppError, CreateRequest};

const STEPS: u8 = 6;
/// Forge tải vài trăm MB thư viện khi cài; mạng chậm cần thời gian.
pub const INSTALLER_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const INSTALLER_HEAP_MB: u32 = 2048;
/// Số dòng log cuối của installer gửi kèm lỗi (spec P3, mục 3.1).
const LOG_TAIL: usize = 40;

impl App {
    pub(crate) async fn install_with_loader(
        &self,
        ctx: &JobContext,
        id: ServerId,
        req: &CreateRequest,
        memory: MemoryLimits,
        port: GamePort,
    ) -> Result<(), AppError> {
        ctx.step(JobStep::CheckDisk, 1, STEPS);
        self.check_disk()?;
        let (paths, game) = self
            .setup_loader_server(
                ctx,
                id,
                req.loader,
                &req.minecraft,
                req.loader_version.as_ref(),
                2,
                STEPS,
            )
            .await?;
        ctx.step(JobStep::PrepareFiles, 6, STEPS);
        self.write_new_server(&paths.server_dir, id, req, game, memory, port)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn setup_loader_server(
        &self,
        ctx: &JobContext,
        id: ServerId,
        loader: LoaderKind,
        mc: &McVersion,
        wanted: Option<&LoaderVersion>,
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
        let install = ctx
            .cancellable(self.catalog.plan(loader, mc, wanted))
            .await?;
        let major = install.java_major;
        ctx.step(JobStep::DownloadJava { major }, step_base + 1, step_count);
        let java = {
            let _permit = ctx.download_permit().await?;
            let mut progress = |done: u64, total: Option<u64>| ctx.bytes(done, total);
            self.java.ensure(major, &mut progress, &ctx.cancel).await?
        };
        ctx.step(
            JobStep::DownloadServer { minecraft },
            step_base + 2,
            step_count,
        );
        let paths = self.store.create_dir(id)?;
        let root = SafeRoot::new(&paths.server_dir)?;
        self.download_plan_files(ctx, &root, &install).await?;
        ctx.step(
            JobStep::RunInstaller {
                loader: loader.to_string(),
            },
            step_base + 3,
            step_count,
        );
        if let Some(run) = &install.plan.installer {
            self.run_installer(ctx, &java.java_bin, &root, run).await?;
        }
        let game = loader_game(loader, &install, &root)?;
        remove_install_dir(&root);
        Ok((paths, game))
    }

    async fn download_plan_files(
        &self,
        ctx: &JobContext,
        root: &SafeRoot,
        install: &LoaderInstall,
    ) -> Result<(), AppError> {
        for file in &install.plan.files {
            let dest = root.join(&file.dest)?;
            let _permit = ctx.download_permit().await?;
            let mut progress = |done: u64, total: Option<u64>| ctx.bytes(done, total);
            self.catalog
                .download_file(file, dest.as_path(), &mut progress, &ctx.cancel)
                .await?;
        }
        ctx.check_cancel()
    }

    async fn run_installer(
        &self,
        ctx: &JobContext,
        java_bin: &Path,
        root: &SafeRoot,
        run: &InstallerRun,
    ) -> Result<(), AppError> {
        // Kiểm tra đường dẫn jar như mọi đường dẫn đến từ nguồn ngoài; truyền dạng tương đối
        // vì thư mục làm việc là thư mục server.
        root.join(&run.jar)?;
        let mut args = vec![format!("-Xmx{INSTALLER_HEAP_MB}M")];
        args.extend(ENCODING_FLAGS.map(str::to_owned));
        args.push("-jar".to_owned());
        args.push(run.jar.clone());
        args.extend(run.args.iter().cloned());
        let spec = ProcessSpec {
            program: java_bin.to_path_buf(),
            args,
            work_dir: root.path().to_path_buf(),
            limits: Some(ResourceLimits::for_heap_mb(INSTALLER_HEAP_MB)),
        };
        let mut tail = VecDeque::with_capacity(LOG_TAIL);
        let done = self
            .runtime
            .run_to_completion(spec, INSTALLER_TIMEOUT, ctx.cancel.cancelled(), |line| {
                tracing::debug!(line = %line.text, "installer");
                if tail.len() == LOG_TAIL {
                    tail.pop_front();
                }
                tail.push_back(line.text);
            })
            .await?;
        match done {
            Completion::Cancelled => Err(AppError::cancelled()),
            done if done.success() => Ok(()),
            done => Err(install_failed("installer không cài xong", done, tail)),
        }
    }
}

fn install_failed(detail: &str, done: Completion, tail: VecDeque<String>) -> AppError {
    let (exit_code, timed_out) = match done {
        Completion::Exited { code } => (code, false),
        _ => (None, true),
    };
    tracing::warn!(?exit_code, timed_out, "cài mod loader thất bại");
    AppError::new(ErrorCode::LoaderInstallFailed, detail).with_params(serde_json::json!({
        "exit_code": exit_code,
        "timed_out": timed_out,
        "log_tail": Vec::from(tail),
    }))
}

/// `[game]` của server mới, sau khi đã thấy file chạy server trên đĩa.
fn loader_game(
    loader: LoaderKind,
    install: &LoaderInstall,
    root: &SafeRoot,
) -> Result<GameSection, AppError> {
    let plan = &install.plan;
    let mut game = GameSection::new(loader, plan.minecraft.clone(), None, install.java_major);
    game.loader_version = Some(plan.loader_version.clone());
    let expected = match &plan.launch {
        Launch::Jar(jar) => {
            game.jar = jar.clone();
            jar.as_str().to_owned()
        }
        Launch::ArgsFile(path) => {
            game.launch = Some(LaunchSection::new(LaunchMode::ArgsFile(path.clone())));
            path.for_os(ArgsOs::current())
        }
    };
    if !root.join(&expected)?.as_path().is_file() {
        return Err(AppError::new(
            ErrorCode::LoaderInstallFailed,
            format!("installer chạy xong nhưng không có `{expected}`"),
        )
        .with_params(serde_json::json!({ "missing": expected })));
    }
    Ok(game)
}

fn remove_install_dir(root: &SafeRoot) {
    let Ok(dir) = root.join(INSTALL_DIR) else {
        return;
    };
    if let Err(e) = std::fs::remove_dir_all(dir.as_path())
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(error = %e, "không xóa được thư mục installer tạm");
    }
}
