//! Máy trạng thái của một server (spec P1, mục 4.2).

use serde::{Deserialize, Serialize};

use crate::error::ErrorCode;

/// Trạng thái server mà UI hiển thị.
///
/// Dữ liệu đi kèm (log cuối, `CrashHint`) do supervisor giữ, để trạng thái luôn là `Copy`
/// và hàm `transition` thuần.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerState {
    /// Job `create` đang tải và chuẩn bị file.
    Installing,
    Stopped,
    Starting,
    Running,
    Stopping,
    /// Process thoát mà người dùng không yêu cầu.
    Crashed,
    /// Thư mục server hỏng hoặc cài lỗi; chỉ còn xem và xóa được.
    Broken,
}

/// Điều xảy ra với server: do người dùng bấm, hoặc do process báo về.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerEvent {
    InstallSucceeded,
    InstallFailed,
    /// Người dùng bấm Bật, hoặc supervisor tự khởi động lại sau crash.
    Start,
    /// Thấy dòng `Done (…)!` trong console.
    Ready,
    Stop,
    Kill,
    /// Process Java đã thoát, vì bất cứ lý do gì.
    ProcessExited,
}

impl ServerState {
    pub const ALL: [Self; 7] = [
        Self::Installing,
        Self::Stopped,
        Self::Starting,
        Self::Running,
        Self::Stopping,
        Self::Crashed,
        Self::Broken,
    ];

    /// Process Java có thể đang sống ở trạng thái này.
    pub const fn has_process(self) -> bool {
        matches!(self, Self::Starting | Self::Running | Self::Stopping)
    }
}

impl ServerEvent {
    pub const ALL: [Self; 7] = [
        Self::InstallSucceeded,
        Self::InstallFailed,
        Self::Start,
        Self::Ready,
        Self::Stop,
        Self::Kill,
        Self::ProcessExited,
    ];
}

/// Trạng thái kế tiếp, hoặc `SERVER_BUSY` nếu sự kiện không hợp lệ lúc này.
///
/// Thoát khi đang `Stopping` là dừng sạch; thoát khi đang `Starting`/`Running` là crash.
/// Chính sách tự khởi động lại (≤ 3 lần / 10 phút) nằm ở supervisor, không ở đây.
pub const fn transition(state: ServerState, event: ServerEvent) -> Result<ServerState, ErrorCode> {
    use ServerEvent as E;
    use ServerState as S;
    match (state, event) {
        (S::Installing, E::InstallSucceeded) => Ok(S::Stopped),
        (S::Installing, E::InstallFailed) => Ok(S::Broken),
        (S::Stopped | S::Crashed, E::Start) => Ok(S::Starting),
        (S::Starting, E::Ready) => Ok(S::Running),
        (S::Starting | S::Running, E::Stop | E::Kill) => Ok(S::Stopping),
        // Đang dừng mà người dùng bấm Buộc dừng: vẫn ở `Stopping`, supervisor kill process.
        (S::Stopping, E::Kill) => Ok(S::Stopping),
        (S::Stopping, E::ProcessExited) => Ok(S::Stopped),
        (S::Starting | S::Running, E::ProcessExited) => Ok(S::Crashed),
        _ => Err(ErrorCode::ServerBusy),
    }
}

/// Dòng báo server đã sẵn sàng, ví dụ `[10:00:00 INFO]: Done (3.2s)! For help, type "help"`.
pub fn is_done_line(line: &str) -> bool {
    line.contains("]: Done (")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ServerEvent as E;
    use ServerState as S;

    /// Mọi cặp hợp lệ. Cặp nào không có ở đây phải trả `SERVER_BUSY`.
    const VALID: [(S, E, S); 11] = [
        (S::Installing, E::InstallSucceeded, S::Stopped),
        (S::Installing, E::InstallFailed, S::Broken),
        (S::Stopped, E::Start, S::Starting),
        (S::Crashed, E::Start, S::Starting),
        (S::Starting, E::Ready, S::Running),
        (S::Starting, E::Stop, S::Stopping),
        (S::Starting, E::Kill, S::Stopping),
        (S::Running, E::Stop, S::Stopping),
        (S::Running, E::Kill, S::Stopping),
        (S::Stopping, E::Kill, S::Stopping),
        (S::Stopping, E::ProcessExited, S::Stopped),
    ];
    const CRASHES: [(S, E, S); 2] = [
        (S::Starting, E::ProcessExited, S::Crashed),
        (S::Running, E::ProcessExited, S::Crashed),
    ];

    #[test]
    fn every_pair_is_covered() {
        let valid: Vec<_> = VALID.iter().chain(CRASHES.iter()).collect();
        let mut checked = 0;
        for state in S::ALL {
            for event in E::ALL {
                let expected = valid
                    .iter()
                    .find(|(s, e, _)| *s == state && *e == event)
                    .map(|(_, _, next)| *next)
                    .ok_or(ErrorCode::ServerBusy);
                assert_eq!(transition(state, event), expected, "{state:?} + {event:?}");
                checked += 1;
            }
        }
        assert_eq!(checked, 49);
    }

    #[test]
    fn broken_accepts_nothing() {
        for event in E::ALL {
            assert_eq!(transition(S::Broken, event), Err(ErrorCode::ServerBusy));
        }
    }

    #[test]
    fn start_twice_is_busy() {
        let starting = transition(S::Stopped, E::Start).unwrap();
        assert_eq!(transition(starting, E::Start), Err(ErrorCode::ServerBusy));
    }

    #[test]
    fn process_only_in_live_states() {
        let live: Vec<_> = S::ALL.into_iter().filter(|s| s.has_process()).collect();
        assert_eq!(live, [S::Starting, S::Running, S::Stopping]);
    }

    #[test]
    fn serde_names() {
        assert_eq!(serde_json::to_string(&S::Crashed).unwrap(), "\"crashed\"");
        let event = E::ProcessExited;
        assert_eq!(serde_json::to_string(&event).unwrap(), "\"process_exited\"");
    }

    #[test]
    fn done_line_detection() {
        assert!(is_done_line(
            "[10:00:00 INFO]: Done (3.214s)! For help, type \"help\""
        ));
        assert!(!is_done_line("[10:00:00 INFO]: Preparing spawn area: 84%"));
        assert!(!is_done_line("<Steve> Done (lol)"));
    }
}
