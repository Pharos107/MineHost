//! Dòng console của server, dùng chung cho runtime, supervisor và API.

use serde::{Deserialize, Serialize};

/// Dòng đến từ đâu. `Stdin` là lệnh người dùng gửi, được ghi lại để console hiện đủ lịch sử.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleStream {
    Stdout,
    Stderr,
    Stdin,
}

/// Một dòng trong bộ đệm console. `seq` tăng dần và không bao giờ lặp, kể cả qua các lần
/// khởi động lại, để client kết nối lại chỉ cần gửi `seq` cuối đã thấy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleLine {
    pub seq: u64,
    pub stream: ConsoleStream,
    pub text: String,
}
