//! Chỉ cho một bản MineHost chạy trên mỗi thư mục dữ liệu (spec P1, mục 5.9).
//!
//! Bản đầu tiên khóa `minehost.lock` và ghi `runtime.json` `{ url, pid }`. Bản sau không khóa
//! được thì đọc `runtime.json` để mở lại trình duyệt tới bản đang chạy rồi thoát.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use mh_platform::DataDirs;
use serde::{Deserialize, Serialize};

const LOCK_FILE: &str = "minehost.lock";
const RUNTIME_FILE: &str = "runtime.json";
/// Bản đầu tiên có thể vừa khóa xong mà chưa mở cổng; chờ nó ghi `runtime.json` tối đa chừng này.
const WAIT_FOR_RUNTIME: Duration = Duration::from_secs(10);

#[derive(Debug, Serialize, Deserialize)]
struct RuntimeInfo {
    url: String,
    pid: u32,
}

/// Giữ khóa tới khi controller tắt. Drop thì xóa `runtime.json`; khóa tự nhả khi file đóng.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
    runtime: PathBuf,
}

#[derive(Debug)]
pub enum Acquired {
    First(InstanceLock),
    /// Đã có bản khác chạy ở địa chỉ này.
    Running {
        url: String,
    },
}

pub fn acquire(dirs: &DataDirs) -> anyhow::Result<Acquired> {
    let path = dirs.root().join(LOCK_FILE);
    let runtime = dirs.root().join(RUNTIME_FILE);
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("không mở được {}", path.display()))?;

    match file.try_lock() {
        Ok(()) => {
            // `runtime.json` còn sót lại từ lần chạy bị kill: xóa trước để bản sau không đọc nhầm.
            remove_if_exists(&runtime);
            Ok(Acquired::First(InstanceLock {
                _file: file,
                runtime,
            }))
        }
        Err(TryLockError::WouldBlock) => Ok(Acquired::Running {
            url: wait_for_url(&runtime)?,
        }),
        Err(TryLockError::Error(err)) => {
            Err(anyhow::Error::new(err).context(format!("không khóa được {}", path.display())))
        }
    }
}

impl InstanceLock {
    /// Ghi địa chỉ thật sau khi đã mở cổng, để bản chạy sau biết mở trình duyệt tới đâu.
    pub fn publish(&self, url: &str) -> anyhow::Result<()> {
        let info = RuntimeInfo {
            url: url.to_owned(),
            pid: std::process::id(),
        };
        let json = serde_json::to_vec_pretty(&info).context("không ghi được runtime.json")?;
        mh_platform::fs::write_atomic(&self.runtime, &json)?;
        Ok(())
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        remove_if_exists(&self.runtime);
    }
}

fn wait_for_url(runtime: &Path) -> anyhow::Result<String> {
    let step = Duration::from_millis(100);
    let mut waited = Duration::ZERO;
    loop {
        if let Some(info) = read_runtime(runtime) {
            return Ok(info.url);
        }
        if waited >= WAIT_FOR_RUNTIME {
            anyhow::bail!(
                "một bản MineHost khác đang chạy nhưng chưa ghi {}",
                runtime.display()
            );
        }
        std::thread::sleep(step);
        waited += step;
    }
}

fn read_runtime(path: &Path) -> Option<RuntimeInfo> {
    let text = std::fs::read(path).ok()?;
    serde_json::from_slice(&text).ok()
}

fn remove_if_exists(path: &Path) {
    if let Err(err) = std::fs::remove_file(path)
        && err.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(%err, path = %path.display(), "không xóa được file");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs() -> (tempfile::TempDir, DataDirs) {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
        dirs.ensure().unwrap();
        (tmp, dirs)
    }

    #[test]
    fn second_acquire_reads_published_url() {
        let (_tmp, dirs) = dirs();
        let Acquired::First(lock) = acquire(&dirs).unwrap() else {
            panic!("bản đầu phải lấy được khóa");
        };
        lock.publish("http://127.0.0.1:7878").unwrap();

        match acquire(&dirs).unwrap() {
            Acquired::Running { url } => assert_eq!(url, "http://127.0.0.1:7878"),
            Acquired::First(_) => panic!("khóa đang bị giữ"),
        }
    }

    #[test]
    fn dropping_lock_frees_it_and_removes_runtime_file() {
        let (_tmp, dirs) = dirs();
        let Acquired::First(lock) = acquire(&dirs).unwrap() else {
            panic!("bản đầu phải lấy được khóa");
        };
        lock.publish("http://127.0.0.1:7878").unwrap();
        drop(lock);

        assert!(!dirs.root().join(RUNTIME_FILE).exists());
        assert!(matches!(acquire(&dirs).unwrap(), Acquired::First(_)));
    }

    #[test]
    fn stale_runtime_file_is_removed_on_acquire() {
        let (_tmp, dirs) = dirs();
        std::fs::write(
            dirs.root().join(RUNTIME_FILE),
            r#"{"url":"http://old","pid":1}"#,
        )
        .unwrap();
        let Acquired::First(_lock) = acquire(&dirs).unwrap() else {
            panic!("không ai giữ khóa");
        };
        assert!(!dirs.root().join(RUNTIME_FILE).exists());
    }
}
