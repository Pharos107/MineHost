//! Việc trước mỗi lần bật server (spec P1, mục 5.6 "Trước khi bật").

use std::path::Path;
use std::sync::Arc;

use mh_core::{ErrorCode, GamePort, Properties, ServerId};
use mh_java::JavaRuntime;
use mh_net::CancellationToken;
use mh_runtime::{JavaLaunch, ProcessSpec};
use mh_store::ServerStore;
use mh_supervisor::{LaunchError, Launcher};

use crate::jobs::{JobKind, JobRunner, JobStep};
use crate::sources::{JavaSource, launch_args};
use crate::{AppError, PortProbe};

pub(crate) const PROPERTIES_FILE: &str = "server.properties";
const EULA_FILE: &str = "eula.txt";

pub(crate) struct AppLauncher {
    pub id: ServerId,
    pub store: ServerStore,
    pub java: Arc<dyn JavaSource>,
    pub jobs: Arc<JobRunner>,
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
        let mut props = read_properties(&paths.server_dir)?;
        props.set("server-port", &port.to_string());
        write_properties(&paths.server_dir, &props)?;
        if !(self.port_free)(port.get()) || bound_ip_busy(&props, port.get()) {
            return Err(port_in_use(port, None));
        }
        for (kind, extra) in extra_ports(&props) {
            if !(self.port_free)(extra) {
                return Err(AppError::new(
                    ErrorCode::PortInUse,
                    format!("cổng {kind} {extra} đang bận"),
                )
                .with_params(serde_json::json!({
                    "port": extra,
                    "suggested_port": null,
                    "kind": kind,
                })));
            }
        }
        let java = self.ensure_java(spec.game.java_major).await?;
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

    /// Đảm bảo bản Java server cần đã có sẵn; nếu thiếu thì chạy qua Job để báo tiến độ tải.
    async fn ensure_java(&self, major: u16) -> Result<JavaRuntime, AppError> {
        if self.java.is_installed(major) {
            return self
                .java
                .ensure(major, &mut |_, _| {}, &CancellationToken::new())
                .await;
        }
        let java = Arc::clone(&self.java);
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.jobs
            .spawn(JobKind::InstallJava, Some(self.id), move |ctx| async move {
                let res = async {
                    let _permit = ctx.download_permit().await?;
                    ctx.step(JobStep::DownloadJava { major }, 1, 1);
                    java.ensure(
                        major,
                        &mut |done, total| ctx.bytes(done, total),
                        &ctx.cancel,
                    )
                    .await
                }
                .await;
                let outcome = res.as_ref().map(|_| ()).map_err(|e| {
                    AppError::new(e.code, e.detail.clone()).with_params(e.params.clone())
                });
                let _ = tx.send(res);
                outcome
            });
        rx.await.unwrap_or_else(|_| Err(AppError::cancelled()))
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

/// Cổng TCP khác mà server sẽ mở theo `server.properties`. Bận thì Java vẫn bật lên rồi tắt hỏng,
/// nên báo trước như cổng game. Cổng query là UDP, không kiểm tra được bằng cách này.
fn extra_ports(props: &Properties) -> Vec<(&'static str, u16)> {
    let mut ports = Vec::new();
    if props.get("enable-rcon").map(str::trim) == Some("true") {
        let rcon = props
            .get("rcon.port")
            .and_then(|p| p.trim().parse().ok())
            .unwrap_or(25575);
        ports.push(("rcon", rcon));
    }
    ports
}

/// `server-ip` đặt một địa chỉ cụ thể thì Java chỉ nghe ở đó. Trên Windows, bind `0.0.0.0` vẫn được
/// khi chương trình khác đã giữ đúng địa chỉ đó, nên thử bind chính nó. Lỗi khác "đang dùng" (địa chỉ
/// không thuộc máy này) thì để Java tự báo.
fn bound_ip_busy(props: &Properties, port: u16) -> bool {
    let Some(ip) = props
        .get("server-ip")
        .and_then(|ip| ip.trim().parse::<std::net::IpAddr>().ok())
    else {
        return false;
    };
    matches!(
        std::net::TcpListener::bind((ip, port)),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rcon_port_is_checked_only_when_enabled() {
        assert!(extra_ports(&Properties::parse("rcon.port=25580\n")).is_empty());
        let on = Properties::parse("enable-rcon=true\nrcon.port=25580\n");
        assert_eq!(extra_ports(&on), [("rcon", 25580)]);
        let default = Properties::parse("enable-rcon=true\n");
        assert_eq!(extra_ports(&default), [("rcon", 25575)]);
    }

    #[test]
    fn specific_server_ip_is_probed_directly() {
        let held = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = held.local_addr().unwrap().port();
        let props = Properties::parse("server-ip=127.0.0.1\n");
        assert!(bound_ip_busy(&props, port));
        assert!(!bound_ip_busy(&Properties::parse("server-ip=\n"), port));
        drop(held);
        assert!(!bound_ip_busy(&props, port));
    }
}
