//! Hash và đường dẫn file tạm. Hash tính trên byte thực sự ghi xuống đĩa.

use std::error::Error;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub(crate) struct HashWriter<W> {
    inner: W,
    hasher: Sha256,
    written: u64,
}

impl<W> HashWriter<W> {
    pub(crate) fn new(inner: W) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            written: 0,
        }
    }

    pub(crate) fn into_parts(self) -> (W, u64, String) {
        let sha = hex(&self.hasher.finalize());
        (self.inner, self.written, sha)
    }
}

impl<W: Write> Write for HashWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.hasher.update(&buf[..n]);
        self.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

/// `<dest>.part` cùng thư mục, để `rename` là một bước trên cùng một ổ.
pub(crate) fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

/// Đánh dấu lỗi đọc khi người dùng hủy, để phân biệt với I/O thật.
#[derive(Debug)]
struct CancelMark;

impl std::fmt::Display for CancelMark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("đã hủy")
    }
}

impl Error for CancelMark {}

/// Không dùng `ErrorKind::Interrupted`: `io::copy` (tar dùng bên trong) gặp lỗi đó thì tự đọc
/// lại, thành vòng lặp vô hạn sau khi đã hủy.
pub(crate) fn cancel_err() -> io::Error {
    io::Error::other(CancelMark)
}

/// `io::Error::source` không trả về lỗi gắn bằng `Error::new`, chỉ trả về
/// `source()` của lỗi đó. Phải xem `get_ref` thì mới thấy `CancelMark`.
pub(crate) fn is_cancel(err: &io::Error) -> bool {
    contains_cancel(err)
}

fn contains_cancel(err: &(dyn Error + 'static)) -> bool {
    if err.downcast_ref::<CancelMark>().is_some() {
        return true;
    }
    if let Some(io_err) = err.downcast_ref::<io::Error>() {
        // Closure để ép `&(dyn Error + Send + Sync)` về `&dyn Error`.
        if io_err.get_ref().is_some_and(|inner| contains_cancel(inner)) {
            return true;
        }
    }
    err.source().is_some_and(contains_cancel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_mark_is_detected_inside_io_error() {
        let err = cancel_err();
        assert!(is_cancel(&err));
        let wrapped = io::Error::other(err);
        assert!(is_cancel(&wrapped));
        assert!(!is_cancel(&io::Error::other("đĩa đầy")));
    }
}
