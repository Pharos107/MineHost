//! Kiểu domain dùng chung của MineHost.
//!
//! Crate này không làm I/O, không dùng async và không phụ thuộc crate nào khác trong workspace,
//! để mọi crate khác đều dùng được và test chạy trong vài mili giây.
//!
//! Mọi kiểu domain chỉ tạo được qua hàm `parse`/`new` có kiểm tra, kể cả khi đọc bằng serde.

pub mod backup_schedule;
pub mod console;
pub mod crash;
pub mod error;
pub mod ids;
pub mod instance;
pub mod launch;
pub mod memory;
pub mod players;
pub mod properties;
pub mod schedule;
pub mod state;
pub mod version;

pub use backup_schedule::{BackupSchedule, check_keep, next_run};
pub use console::{ConsoleLine, ConsoleStream};
pub use crash::{CrashHint, diagnose};
pub use error::{ErrorCode, InvalidValue};
pub use ids::{GamePort, ServerId, ServerName};
pub use instance::{InstanceError, InstanceSpec};
pub use launch::{ArgsFilePath, ArgsOs, Launch};
pub use memory::{MemoryLimits, suggest_memory_mb};
pub use players::{
    FreezeAttribute, PlayerAction, PlayerCommandError, PlayerLogEvent, PlayerName,
    UnsupportedVersion, command, freeze_attributes, offline_uuid, parse_log_line,
};
pub use properties::Properties;
pub use schedule::{DailyTime, MAX_RESTART_TIMES, normalize_restart_times, seconds_until_next};
pub use state::{ServerEvent, ServerState, transition};
pub use version::{ContentKind, LoaderKind, LoaderVersion, McVersion};

/// Tên sản phẩm hiển thị cho người dùng.
pub const PRODUCT_NAME: &str = "MineHost";
