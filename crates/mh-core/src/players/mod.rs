//! Người chơi: đọc một dòng log, UUID offline và lệnh console.
//!
//! Hàm thuần, không I/O. Supervisor ghép log thật vào [`parse_log_line`]; app gửi
//! chuỗi từ [`command`] vào stdin của server (spec P2, mục 3.2, 5.1, 6.7).

mod command;
mod freeze;
mod log;
mod name;

pub use command::{PlayerAction, PlayerCommandError, command};
pub use freeze::{FreezeAttribute, UnsupportedVersion, freeze_attributes};
pub use log::{PlayerLogEvent, parse_log_line};
pub use name::{PlayerName, offline_uuid};

#[cfg(test)]
mod tests;
