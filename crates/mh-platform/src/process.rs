//! CPU/RAM của một process và tổng kích thước thư mục.
//!
//! `sysinfo` tính CPU bằng hiệu hai lần đọc, nên lần `usage` đầu của một pid thường
//! trả `cpu_percent = 0`. Giữ [`ProcessProbe`] rồi gọi lại (cách nhau ít nhất ~200 ms)
//! thì số mới phản ánh khoảng vừa rồi.

use std::fs::{self, DirEntry};
use std::path::Path;

use sysinfo::{Pid, ProcessesToUpdate, System};

/// CPU và RSS của một process tại một lần đọc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessUsage {
    /// Phần trăm trên toàn máy, trong khoảng 0..=100 (đã chia cho số luồng CPU).
    pub cpu_percent: f32,
    /// Resident set size, byte.
    pub rss_bytes: u64,
}

/// Bộ đo giữ `System` của sysinfo để lần đọc sau của cùng một pid có mốc so sánh.
#[derive(Debug)]
pub struct ProcessProbe {
    system: System,
}

impl ProcessProbe {
    pub fn new() -> Self {
        let mut system = System::new();
        // Cần danh sách CPU để đổi % theo từng lõi (sysinfo có thể > 100) thành % toàn máy.
        system.refresh_cpu_all();
        Self { system }
    }

    /// `None` khi process không còn. Lần đầu gặp `pid` thì `cpu_percent` thường là 0.
    pub fn usage(&mut self, pid: u32) -> Option<ProcessUsage> {
        let pid = Pid::from_u32(pid);
        self.system
            .refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        let process = self.system.process(pid)?;
        Some(ProcessUsage {
            cpu_percent: whole_machine_percent(process.cpu_usage(), self.cpu_threads()),
            rss_bytes: process.memory(),
        })
    }

    fn cpu_threads(&self) -> f32 {
        let threads = self.system.cpus().len().max(1);
        threads as f32
    }
}

impl Default for ProcessProbe {
    fn default() -> Self {
        Self::new()
    }
}

/// `raw` của sysinfo là % theo từng lõi (có thể vượt 100). Chia cho số luồng ra % toàn máy.
fn whole_machine_percent(raw: f32, threads: f32) -> f32 {
    if !raw.is_finite() || threads <= 0.0 {
        return 0.0;
    }
    (raw / threads).clamp(0.0, 100.0)
}

/// Tổng byte của file thường dưới `path`.
///
/// Không theo symlink, kể cả symlink trỏ tới thư mục. File hoặc thư mục không đọc được thì
/// bỏ qua, không làm hỏng cả lần tính. `path` không tồn tại thì trả 0.
pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    add_tree(path, &mut total);
    total
}

fn add_tree(path: &Path, total: &mut u64) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        add_entry(&entry, total);
    }
}

fn add_entry(entry: &DirEntry, total: &mut u64) {
    let Ok(kind) = entry.file_type() else {
        return;
    };
    // `file_type` không đi theo symlink. Bỏ hẳn để không cộng kích thước đích.
    if kind.is_symlink() {
        return;
    }
    if kind.is_dir() {
        add_tree(&entry.path(), total);
        return;
    }
    if !kind.is_file() {
        return;
    }
    let Ok(meta) = entry.metadata() else {
        return;
    };
    *total = total.saturating_add(meta.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_of_current_process_has_rss() {
        let mut probe = ProcessProbe::new();
        let first = probe.usage(std::process::id()).unwrap();
        let second = probe.usage(std::process::id()).unwrap();
        assert!(first.rss_bytes > 0, "{first:?}");
        assert!(second.rss_bytes > 0, "{second:?}");
        assert!((0.0..=100.0).contains(&first.cpu_percent), "{first:?}");
        assert!((0.0..=100.0).contains(&second.cpu_percent), "{second:?}");
    }

    #[test]
    fn dir_size_sums_file_bytes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        std::fs::write(dir.path().join("b.txt"), b"hello").unwrap();
        assert_eq!(dir_size(dir.path()), 8);
        assert_eq!(dir_size(&dir.path().join("missing")), 0);
    }

    #[cfg(unix)]
    #[test]
    fn dir_size_skips_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"abc").unwrap();
        std::fs::write(dir.path().join("b.txt"), b"hello").unwrap();
        std::fs::write(outside.path().join("big"), vec![1u8; 64]).unwrap();
        std::fs::create_dir(outside.path().join("nested")).unwrap();
        std::fs::write(outside.path().join("nested").join("x"), b"0123456789").unwrap();
        std::os::unix::fs::symlink(outside.path().join("big"), dir.path().join("link")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("nested"), dir.path().join("dirlink"))
            .unwrap();
        assert_eq!(dir_size(dir.path()), 8);
    }
}
