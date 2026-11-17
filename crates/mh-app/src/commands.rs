//! Danh sách lệnh của server lấy bằng `help`, để ô lệnh console gợi ý cả lệnh của mod và plugin
//! (spec `lenh-cua-server.md`).

use std::time::Duration;

use mh_core::commands::{HelpLine, ServerCommand, help_requests, merge_help, parse_help_line};
use mh_core::{ConsoleStream, ErrorCode, ServerId, ServerState};
use mh_supervisor::SupervisorEvent;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::broadcast::{Receiver, error::RecvError};
use tokio::time::{Instant, timeout_at};

use crate::{App, AppError};

/// Nằm cạnh `instance.toml`, ngoài thư mục game.
const FILE: &str = "commands.json";
/// `help` in hết danh sách trong cùng một tick; im lặng chừng này là đã xong.
const QUIET: Duration = Duration::from_millis(1500);
const LIMIT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandList {
    #[serde(with = "time::serde::rfc3339::option")]
    pub scanned_at: Option<OffsetDateTime>,
    pub commands: Vec<ServerCommand>,
}

impl App {
    /// Danh sách đã quét lần trước; chưa quét (hoặc file hỏng) thì rỗng.
    pub fn server_commands(&self, id: ServerId) -> Result<CommandList, AppError> {
        self.store.load(id)?;
        let path = self.store.paths(id).root.join(FILE);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_default()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(CommandList::default())
            }
            Err(error) => Err(AppError::new(
                ErrorCode::Internal,
                format!("đọc {FILE}: {error}"),
            )),
        }
    }

    /// Gửi `help`, đọc các dòng trả về rồi lưu. Lệnh hiện trong console như người dùng gõ.
    pub async fn scan_commands(&self, id: ServerId) -> Result<CommandList, AppError> {
        let spec = self.store.load(id)?;
        let handle = self.handle(id)?;
        if handle.status().state != ServerState::Running {
            return Err(AppError::new(
                ErrorCode::ServerNotRunning,
                "quét lệnh cần server đang chạy",
            ));
        }
        // Đăng ký trước khi gửi để không lỡ dòng trả lời nào.
        let mut events = handle.subscribe();
        for request in help_requests(spec.game.loader) {
            handle.send(*request).await?;
        }
        let list = CommandList {
            scanned_at: Some(OffsetDateTime::now_utc()),
            commands: merge_help(collect(&mut events).await),
        };
        let bytes = serde_json::to_vec_pretty(&list)
            .map_err(|error| AppError::new(ErrorCode::Internal, error.to_string()))?;
        mh_platform::fs::write_atomic(&self.store.paths(id).root.join(FILE), &bytes)?;
        // Quét luôn registry của mods/*.jar để gợi ý đồ/hiệu ứng mới nếu vừa thêm mod.
        if let Err(error) = self.scan_registry(id).await {
            tracing::warn!(server = %id, %error, "không quét được registry từ mods/");
        }
        tracing::info!(server = %id, count = list.commands.len(), "đã quét lệnh của server");
        Ok(list)
    }
}

/// Gom dòng `help` cho tới khi im lặng [`QUIET`] sau dòng cuối, hoặc hết [`LIMIT`].
async fn collect(events: &mut Receiver<SupervisorEvent>) -> Vec<HelpLine> {
    let end = Instant::now() + LIMIT;
    let mut found = Vec::new();
    let mut last: Option<Instant> = None;
    loop {
        let deadline = last.map_or(end, |at| (at + QUIET).min(end));
        match timeout_at(deadline, events.recv()).await {
            Ok(Ok(SupervisorEvent::Console(line))) if line.stream == ConsoleStream::Stdout => {
                if let Some(help) = parse_help_line(&line.text) {
                    found.push(help);
                    last = Some(Instant::now());
                }
            }
            // Bị bỏ lại vài sự kiện thì vẫn dùng phần đã đọc được.
            Ok(Ok(_)) | Ok(Err(RecvError::Lagged(_))) => {}
            Ok(Err(RecvError::Closed)) | Err(_) => break,
        }
    }
    found
}
