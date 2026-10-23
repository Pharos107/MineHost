//! Kiểu domain dùng chung của MineHost.
//!
//! Crate này không làm I/O, không dùng async và không phụ thuộc crate nào khác trong workspace,
//! để mọi crate khác đều dùng được và test chạy trong vài mili giây.

pub mod error;

pub use error::ErrorCode;

/// Tên sản phẩm hiển thị cho người dùng.
pub const PRODUCT_NAME: &str = "MineHost";
