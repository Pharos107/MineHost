//! Kiểu domain dùng chung của MineHost.
//!
//! Crate này không làm I/O, không dùng async và không phụ thuộc crate nào khác trong workspace,
//! để mọi crate khác đều dùng được và test chạy trong vài mili giây.
//!
//! Mọi kiểu domain chỉ tạo được qua hàm `parse`/`new` có kiểm tra, kể cả khi đọc bằng serde.

pub mod console;
pub mod crash;
pub mod error;
pub mod ids;
pub mod instance;
pub mod memory;
pub mod properties;
pub mod state;
pub mod version;

pub use console::{ConsoleLine, ConsoleStream};
pub use crash::{CrashHint, diagnose};
pub use error::{ErrorCode, InvalidValue};
pub use ids::{GamePort, ServerId, ServerName};
pub use instance::{InstanceError, InstanceSpec};
pub use memory::{MemoryLimits, suggest_memory_mb};
pub use properties::Properties;
pub use state::{ServerEvent, ServerState, transition};
pub use version::{LoaderKind, McVersion};

/// Tên sản phẩm hiển thị cho người dùng.
pub const PRODUCT_NAME: &str = "MineHost";
