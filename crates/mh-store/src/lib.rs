//! Thư mục server trên đĩa: nguồn dữ liệu chính của MineHost (P1 chưa dùng SQLite, mục 3.1).
//!
//! Mỗi server nằm trong `servers/<id>/`:
//!
//! ```text
//! servers/<id>/instance.toml   cấu hình của MineHost
//! servers/<id>/server/         thư mục làm việc của Java (world, server.properties, …)
//! servers/<id>/logs/           log console theo ngày
//! servers/<id>/.running        có mặt khi server đang bật; controller chết thì còn lại
//! ```

mod error;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use mh_core::{InstanceError, InstanceSpec, ServerId};
use serde::Serialize;
use time::OffsetDateTime;
use time::macros::format_description;

pub use error::StoreError;

const INSTANCE_FILE: &str = "instance.toml";
const RUNNING_MARKER: &str = ".running";

#[derive(Debug, Clone)]
pub struct ServerStore {
    servers_dir: PathBuf,
    trash_dir: PathBuf,
}

/// Các đường dẫn của một server. `id` đã là ULID hợp lệ nên ghép vào đường dẫn an toàn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerPaths {
    pub root: PathBuf,
    pub instance_file: PathBuf,
    pub server_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub running_marker: PathBuf,
}

/// Một thư mục tìm thấy khi quét `servers/`.
#[derive(Debug, Clone, PartialEq)]
pub struct Listed {
    pub id: ServerId,
    /// Có file `.running`: lần trước server đang chạy thì controller dừng đột ngột.
    pub marked_running: bool,
    pub spec: Result<InstanceSpec, BrokenReason>,
}

/// Vì sao một thư mục server không dùng được. UI dịch theo `kind`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BrokenReason {
    #[error("thiếu instance.toml")]
    MissingInstance,
    #[error("không đọc được instance.toml: {detail}")]
    Unreadable { detail: String },
    #[error("instance.toml không hợp lệ: {detail}")]
    Invalid { detail: String },
    /// Tạo bởi bản MineHost mới hơn.
    #[error("instance.toml có schema {found}, mới hơn bản MineHost này")]
    NewerSchema { found: i64 },
    /// `id` trong file khác tên thư mục, thường do người dùng chép tay thư mục server.
    #[error("id trong instance.toml là {found}, khác tên thư mục")]
    IdMismatch { found: String },
}

impl ServerStore {
    pub fn new(servers_dir: impl Into<PathBuf>, trash_dir: impl Into<PathBuf>) -> Self {
        Self {
            servers_dir: servers_dir.into(),
            trash_dir: trash_dir.into(),
        }
    }

    pub fn paths(&self, id: ServerId) -> ServerPaths {
        let root = self.servers_dir.join(id.to_string());
        ServerPaths {
            instance_file: root.join(INSTANCE_FILE),
            server_dir: root.join("server"),
            logs_dir: root.join("logs"),
            running_marker: root.join(RUNNING_MARKER),
            root,
        }
    }

    /// Quét `servers/*/`, theo thứ tự tạo (ULID tăng dần theo thời gian).
    ///
    /// Thư mục có tên không phải ULID bị bỏ qua: không có ID thì không thao tác được với nó.
    pub fn list(&self) -> Result<Vec<Listed>, StoreError> {
        let entries = match fs::read_dir(&self.servers_dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(StoreError::io(&self.servers_dir, e)),
        };
        let mut listed = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| StoreError::io(&self.servers_dir, e))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(id) = ServerId::parse(&name) else {
                tracing::debug!(name, "bỏ qua thư mục không phải server");
                continue;
            };
            if !entry.path().is_dir() {
                continue;
            }
            let paths = self.paths(id);
            listed.push(Listed {
                id,
                marked_running: paths.running_marker.exists(),
                spec: read_spec(id, &paths.instance_file),
            });
        }
        listed.sort_by_key(|l| l.id);
        Ok(listed)
    }

    pub fn load(&self, id: ServerId) -> Result<InstanceSpec, StoreError> {
        let paths = self.paths(id);
        if !paths.root.is_dir() {
            return Err(StoreError::NotFound(id));
        }
        read_spec(id, &paths.instance_file).map_err(|reason| StoreError::Broken { id, reason })
    }

    /// Ghi `instance.toml` theo kiểu nguyên tử. Thư mục server phải có sẵn.
    pub fn save(&self, spec: &InstanceSpec) -> Result<(), StoreError> {
        let paths = self.paths(spec.id);
        if !paths.root.is_dir() {
            return Err(StoreError::NotFound(spec.id));
        }
        let text = spec.to_toml().map_err(StoreError::Serialize)?;
        mh_platform::fs::write_atomic(&paths.instance_file, text.as_bytes())?;
        Ok(())
    }

    /// Tạo `servers/<id>/` cùng `server/` và `logs/`. Báo lỗi nếu thư mục đã có.
    pub fn create_dir(&self, id: ServerId) -> Result<ServerPaths, StoreError> {
        let paths = self.paths(id);
        fs::create_dir_all(&self.servers_dir).map_err(|e| StoreError::io(&self.servers_dir, e))?;
        match fs::create_dir(&paths.root) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                return Err(StoreError::AlreadyExists(id));
            }
            Err(e) => return Err(StoreError::io(&paths.root, e)),
        }
        for dir in [&paths.server_dir, &paths.logs_dir] {
            fs::create_dir_all(dir).map_err(|e| StoreError::io(dir, e))?;
        }
        Ok(paths)
    }

    /// Xóa thư mục vừa tạo dở (Job `create` lỗi hoặc bị hủy). Không có thì thôi.
    pub fn remove_unfinished(&self, id: ServerId) -> Result<(), StoreError> {
        let root = self.paths(id).root;
        match fs::remove_dir_all(&root) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(StoreError::io(&root, e)),
            _ => Ok(()),
        }
    }

    /// Chuyển server vào `trash/<id>-<thời điểm UTC>/` và trả về đường dẫn mới.
    pub fn trash(&self, id: ServerId, now: OffsetDateTime) -> Result<PathBuf, StoreError> {
        let root = self.paths(id).root;
        if !root.is_dir() {
            return Err(StoreError::NotFound(id));
        }
        let stamp = now
            .to_offset(time::UtcOffset::UTC)
            .format(format_description!(
                "[year][month][day]-[hour][minute][second]"
            ))
            .map_err(|e| StoreError::io(&root, io::Error::other(e)))?;
        fs::create_dir_all(&self.trash_dir).map_err(|e| StoreError::io(&self.trash_dir, e))?;
        let dest = self.trash_dir.join(format!("{id}-{stamp}"));
        // Cùng ổ đĩa với `servers/` (đều trong thư mục dữ liệu) nên `rename` không phải chép.
        fs::rename(&root, &dest).map_err(|e| StoreError::io(&root, e))?;
        Ok(dest)
    }

    pub fn mark_running(&self, id: ServerId) -> Result<(), StoreError> {
        let marker = self.paths(id).running_marker;
        fs::write(&marker, b"").map_err(|e| StoreError::io(&marker, e))
    }

    pub fn clear_running(&self, id: ServerId) -> Result<(), StoreError> {
        let marker = self.paths(id).running_marker;
        match fs::remove_file(&marker) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(StoreError::io(&marker, e)),
            _ => Ok(()),
        }
    }
}

fn read_spec(id: ServerId, file: &Path) -> Result<InstanceSpec, BrokenReason> {
    let text = match fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(BrokenReason::MissingInstance),
        Err(e) => {
            return Err(BrokenReason::Unreadable {
                detail: e.to_string(),
            });
        }
    };
    let spec = InstanceSpec::from_toml(&text).map_err(|e| match e {
        InstanceError::NewerSchema { found } => BrokenReason::NewerSchema { found },
        other => BrokenReason::Invalid {
            detail: other.to_string(),
        },
    })?;
    if spec.id != id {
        return Err(BrokenReason::IdMismatch {
            found: spec.id.to_string(),
        });
    }
    Ok(spec)
}

#[cfg(test)]
mod tests;
