//! `ServerService`: các thao tác trên từng server (spec P1, mục 5.7).

use std::path::PathBuf;
use std::sync::Arc;

use mh_core::{ErrorCode, GamePort, InstanceSpec, MemoryLimits, ServerId, ServerName, ServerState};
use mh_store::BrokenReason;
use mh_supervisor::{Status, SupervisorHandle};
use time::OffsetDateTime;

use crate::launcher::{AppLauncher, port_in_use, read_properties, write_eula, write_properties};
use crate::{App, AppError, Installing, lock};

/// Một server như UI thấy: đang cài, hỏng, hoặc đã cài kèm trạng thái chạy.
#[derive(Debug, Clone)]
pub struct ServerView {
    pub id: ServerId,
    pub state: ServerState,
    pub spec: Option<InstanceSpec>,
    pub installing: Option<Installing>,
    pub broken: Option<BrokenReason>,
    pub status: Option<Status>,
}

/// Các cài đặt sửa được trong tab "Cài đặt server". Có hiệu lực từ lần bật sau.
#[derive(Debug, Clone, Default)]
pub struct SettingsPatch {
    pub name: Option<ServerName>,
    pub memory_mb: Option<MemoryLimits>,
    pub port: Option<GamePort>,
    pub autostart: Option<bool>,
    pub restart_on_crash: Option<bool>,
}

impl App {
    pub fn list(&self) -> Result<Vec<ServerView>, AppError> {
        let installing = lock(&self.installing).clone();
        let mut views: Vec<ServerView> = installing
            .iter()
            .map(|(id, info)| installing_view(*id, info.clone()))
            .collect();
        for listed in self.store.list()? {
            if installing.contains_key(&listed.id) {
                continue;
            }
            views.push(match listed.spec {
                Ok(spec) => self.ready_view(spec),
                Err(reason) => broken_view(listed.id, reason),
            });
        }
        views.sort_by_key(|v| v.id);
        Ok(views)
    }

    pub fn get(&self, id: ServerId) -> Result<ServerView, AppError> {
        if let Some(info) = lock(&self.installing).get(&id) {
            return Ok(installing_view(id, info.clone()));
        }
        match self.store.load(id) {
            Ok(spec) => Ok(self.ready_view(spec)),
            Err(mh_store::StoreError::Broken { reason, .. }) => Ok(broken_view(id, reason)),
            Err(e) => Err(e.into()),
        }
    }

    pub fn update_settings(
        &self,
        id: ServerId,
        patch: SettingsPatch,
    ) -> Result<InstanceSpec, AppError> {
        let mut spec = self.store.load(id)?;
        if let Some(port) = patch.port.filter(|p| *p != spec.network.port) {
            self.ensure_port_unused(port, Some(id))?;
            spec.network.port = port;
        }
        if let Some(name) = patch.name {
            spec.name = name;
        }
        if let Some(memory) = patch.memory_mb {
            spec.runtime.memory_mb = memory;
        }
        if let Some(autostart) = patch.autostart {
            spec.policy.autostart = autostart;
        }
        if let Some(restart) = patch.restart_on_crash {
            spec.policy.restart_on_crash = restart;
        }
        self.store.save(&spec)?;
        Ok(spec)
    }

    /// Người dùng bấm "Tôi đồng ý". Đây là nơi duy nhất ghi `eula=true` lần đầu.
    pub fn accept_eula(&self, id: ServerId) -> Result<InstanceSpec, AppError> {
        let mut spec = self.store.load(id)?;
        write_eula(&self.store.paths(id).server_dir)?;
        if spec.eula.accepted_at.is_none() {
            let now = OffsetDateTime::now_utc();
            spec.eula.accepted_at = Some(now.replace_nanosecond(0).unwrap_or(now));
            self.store.save(&spec)?;
        }
        Ok(spec)
    }

    /// Chuyển server đã tắt vào thùng rác. `confirm_name` phải trùng tên server (hoặc ID nếu
    /// server hỏng), để không xóa nhầm.
    pub fn trash(&self, id: ServerId, confirm_name: &str) -> Result<PathBuf, AppError> {
        let view = self.get(id)?;
        if view.installing.is_some() || view.state.has_process() {
            return Err(AppError::new(
                ErrorCode::ServerBusy,
                "chỉ xóa được server đang tắt",
            ));
        }
        if view.status.as_ref().is_some_and(|s| s.restart_pending) {
            return Err(AppError::new(
                ErrorCode::ServerBusy,
                "server đang chờ tự bật lại",
            ));
        }
        let expected = view
            .spec
            .map_or_else(|| id.to_string(), |s| s.name.to_string());
        if confirm_name.trim() != expected {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "tên xác nhận không khớp",
            ));
        }
        lock(&self.supervisors).remove(&id);
        let dest = self.store.trash(id, OffsetDateTime::now_utc())?;
        tracing::info!(server = %id, dest = %dest.display(), "đã chuyển server vào thùng rác");
        Ok(dest)
    }

    pub async fn start(&self, id: ServerId) -> Result<(), AppError> {
        Ok(self.handle(id)?.start().await?)
    }

    pub async fn stop(&self, id: ServerId) -> Result<(), AppError> {
        Ok(self.handle(id)?.stop().await?)
    }

    pub async fn restart(&self, id: ServerId) -> Result<(), AppError> {
        Ok(self.handle(id)?.restart().await?)
    }

    pub async fn kill(&self, id: ServerId) -> Result<(), AppError> {
        Ok(self.handle(id)?.kill().await?)
    }

    pub async fn send_command(&self, id: ServerId, command: &str) -> Result<(), AppError> {
        Ok(self.handle(id)?.send(command).await?)
    }

    /// Supervisor của server đã cài; dùng cho console và đăng ký sự kiện.
    pub fn handle(&self, id: ServerId) -> Result<SupervisorHandle, AppError> {
        if lock(&self.installing).contains_key(&id) {
            return Err(AppError::new(ErrorCode::ServerBusy, "server đang được cài"));
        }
        self.store.load(id)?;
        Ok(self.supervisor(id))
    }

    pub fn read_properties(&self, id: ServerId) -> Result<Vec<(String, String)>, AppError> {
        self.store.load(id)?;
        let props = read_properties(&self.store.paths(id).server_dir)?;
        Ok(props
            .entries()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect())
    }

    /// Thay toàn bộ khóa–giá trị; khóa không còn trong danh sách bị xóa. Comment được giữ.
    pub fn write_properties(
        &self,
        id: ServerId,
        entries: &[(String, String)],
    ) -> Result<(), AppError> {
        self.store.load(id)?;
        if entries.iter().any(|(k, _)| k.trim().is_empty()) {
            return Err(AppError::new(ErrorCode::InvalidInput, "khóa rỗng"));
        }
        let dir = self.store.paths(id).server_dir;
        let mut props = read_properties(&dir)?;
        let old_keys: Vec<String> = props.entries().map(|(k, _)| k.to_owned()).collect();
        for key in old_keys {
            if !entries.iter().any(|(k, _)| *k == key) {
                props.remove(&key);
            }
        }
        for (key, value) in entries {
            props.set(key, value);
        }
        write_properties(&dir, &props)
    }

    /// Supervisor của server, tạo mới nếu chưa có.
    pub(crate) fn supervisor(&self, id: ServerId) -> SupervisorHandle {
        let mut supervisors = lock(&self.supervisors);
        let handle = supervisors.entry(id).or_insert_with(|| {
            let mut config = self.supervisor_config.clone();
            config.logs_dir = Some(self.store.paths(id).logs_dir);
            let launcher = AppLauncher {
                id,
                store: self.store.clone(),
                java: Arc::clone(&self.java),
                port_free: Arc::clone(&self.port_free),
            };
            mh_supervisor::spawn(config, Arc::clone(&self.runtime), launcher)
        });
        handle.clone()
    }

    pub(crate) fn supervisor_handles(&self) -> Vec<(ServerId, SupervisorHandle)> {
        lock(&self.supervisors)
            .iter()
            .map(|(id, h)| (*id, h.clone()))
            .collect()
    }

    /// Cổng do server khác của MineHost đang giữ (kể cả server đang cài).
    fn ports_in_use(&self, except: Option<ServerId>) -> Vec<GamePort> {
        let mut ports: Vec<GamePort> = lock(&self.installing)
            .iter()
            .filter(|(id, _)| Some(**id) != except)
            .map(|(_, i)| i.port)
            .collect();
        if let Ok(listed) = self.store.list() {
            ports.extend(
                listed
                    .into_iter()
                    .filter(|l| Some(l.id) != except)
                    .filter_map(|l| l.spec.ok().map(|s| s.network.port)),
            );
        }
        ports
    }

    pub(crate) fn ensure_port_unused(
        &self,
        port: GamePort,
        except: Option<ServerId>,
    ) -> Result<(), AppError> {
        if self.ports_in_use(except).contains(&port) {
            return Err(port_in_use(port, self.choose_port(except).ok()));
        }
        Ok(())
    }

    /// Cổng trống đầu tiên từ 25565 tới 25600 (spec P1, mục 4.4).
    pub(crate) fn choose_port(&self, except: Option<ServerId>) -> Result<GamePort, AppError> {
        let used = self.ports_in_use(except);
        (GamePort::DEFAULT.get()..=GamePort::AUTO_LAST.get())
            .filter_map(|p| GamePort::new(p).ok())
            .find(|p| !used.contains(p) && (self.port_free)(p.get()))
            .ok_or_else(|| port_in_use(GamePort::DEFAULT, None))
    }

    fn ready_view(&self, spec: InstanceSpec) -> ServerView {
        let status = self.supervisor(spec.id).status();
        ServerView {
            id: spec.id,
            state: status.state,
            spec: Some(spec),
            installing: None,
            broken: None,
            status: Some(status),
        }
    }
}

fn installing_view(id: ServerId, info: Installing) -> ServerView {
    ServerView {
        id,
        state: ServerState::Installing,
        spec: None,
        installing: Some(info),
        broken: None,
        status: None,
    }
}

fn broken_view(id: ServerId, reason: BrokenReason) -> ServerView {
    ServerView {
        id,
        state: ServerState::Broken,
        spec: None,
        installing: None,
        broken: Some(reason),
        status: None,
    }
}
