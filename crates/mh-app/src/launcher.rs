//! Việc trước mỗi lần bật server (spec P1, mục 5.6 "Trước khi bật").

use std::path::Path;
use std::sync::Arc;

use mh_core::{ErrorCode, GamePort, Properties, ServerId};
use mh_net::CancellationToken;
use mh_runtime::{JavaLaunch, ProcessSpec};
use mh_store::ServerStore;
use mh_supervisor::{LaunchError, Launcher};

use crate::sources::{JavaSource, launch_args};
use crate::{AppError, PortProbe};

pub(crate) const PROPERTIES_FILE: &str = "server.properties";
const EULA_FILE: &str = "eula.txt";

pub(crate) struct AppLauncher {
    pub id: ServerId,
    pub store: ServerStore,
    pub java: Arc<dyn JavaSource>,
    pub port_free: PortProbe,
}

impl Launcher for AppLauncher {
    async fn prepare(&self) -> Result<ProcessSpec, LaunchError> {
        self.prepare_inner().await.map_err(to_launch)
    }

    fn restart_on_crash(&self) -> bool {
        self.store
            .load(self.id)
            .map_or(true, |s| s.policy.restart_on_crash)
    }

    fn set_running_marker(&self, running: bool) {
        let result = if running {
            self.store.mark_running(self.id)
        } else {
            self.store.clear_running(self.id)
        };
        if let Err(e) = result {
            tracing::warn!(server = %self.id, error = %e, "không cập nhật được dấu .running");
        }
    }
}

impl AppLauncher {
    async fn prepare_inner(&self) -> Result<ProcessSpec, AppError> {
        let spec = self.store.load(self.id)?;
        if !spec.eula_accepted() {
            return Err(AppError::new(
                ErrorCode::EulaNotAccepted,
                "chưa đồng ý EULA",
            ));
        }
        let paths = self.store.paths(self.id);
        // Người dùng đã đồng ý (có `accepted_at`), nên ghi lại `eula.txt` nếu nó bị xóa là đúng ý họ.
        write_eula(&paths.server_dir)?;
        let port = spec.network.port;
        set_properties(&paths.server_dir, &[("server-port", &port.to_string())])?;
        if !(self.port_free)(port.get()) {
            return Err(port_in_use(port, None));
        }
        let java = self
            .java
            .ensure(
                spec.game.java_major,
                &mut |_, _| {},
                &CancellationToken::new(),
            )
            .await?;
        Ok(JavaLaunch {
            java_bin: java.java_bin,
            work_dir: paths.server_dir,
            memory: spec.runtime.memory_mb,
            jvm_args: spec.runtime.jvm_args,
            launch: spec.game.launch(),
            args: launch_args(spec.game.loader),
        }
        .into_spec())
    }
}

fn to_launch(err: AppError) -> LaunchError {
    LaunchError::new(err.code, err.detail).with_params(err.params)
}

pub(crate) fn port_in_use(port: GamePort, suggested: Option<GamePort>) -> AppError {
    let params = serde_json::json!({
        "port": port.get(),
        "suggested_port": suggested.map(GamePort::get),
    });
    AppError::new(ErrorCode::PortInUse, format!("cổng {port} đang bận")).with_params(params)
}

/// Chỉ gọi sau khi người dùng đã bấm đồng ý EULA.
pub(crate) fn write_eula(server_dir: &Path) -> Result<(), AppError> {
    let text =
        "# Người dùng đã đồng ý EULA trong MineHost: https://aka.ms/MinecraftEULA\neula=true\n";
    mh_platform::fs::write_atomic(&server_dir.join(EULA_FILE), text.as_bytes())?;
    Ok(())
}

pub(crate) fn read_properties(server_dir: &Path) -> Result<Properties, AppError> {
    let path = server_dir.join(PROPERTIES_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Properties::parse(&text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Properties::default()),
        Err(e) => Err(AppError::new(
            ErrorCode::Internal,
            format!("{}: {e}", path.display()),
        )),
    }
}

pub(crate) fn write_properties(server_dir: &Path, props: &Properties) -> Result<(), AppError> {
    let path = server_dir.join(PROPERTIES_FILE);
    mh_platform::fs::write_atomic(&path, props.to_string().as_bytes())?;
    Ok(())
}

/// Sửa vài khóa, giữ nguyên phần còn lại của file.
pub(crate) fn set_properties(server_dir: &Path, entries: &[(&str, &str)]) -> Result<(), AppError> {
    let mut props = read_properties(server_dir)?;
    for (key, value) in entries {
        props.set(key, value);
    }
    write_properties(server_dir, &props)
}
