//! Client HTTP dùng chung: User-Agent, timeout, retry, cache, tải file có kiểm tra hash.
//!
//! Mọi lời gọi mạng của MineHost đi qua đây, để retry, cache và thông báo lỗi giống nhau ở mọi nơi.

mod client;
mod download;
mod error;
pub mod hosts;

pub use client::{Fetched, FreeSpaceFn, HttpClient, NetConfig, USER_AGENT};
pub use download::{Checksum, Download, MIN_FREE_BYTES};
pub use error::NetError;
pub use hosts::HostAllowlist;
pub use tokio_util::sync::CancellationToken;
