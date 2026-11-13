//! cgroup v2 cho từng server trên Linux (spec P3, mục 3.8). Không có `unsafe`: phần chạy giữa
//! fork và exec nằm trong `linux.rs`.
//!
//! cgroup v2 không cho một cgroup vừa chứa process vừa giao controller cho cgroup con. Vì vậy
//! controller tự chuyển vào `<cgroup của nó>/minehost-controller`, bật `memory` và `pids` cho
//! cgroup cũ, rồi tạo một cgroup anh em cho mỗi server. Chỉ làm được khi cgroup hiện tại được
//! giao quyền cho user (unit systemd có `Delegate=yes`, hoặc `systemd-run --user`); không thì
//! chạy như trước và ghi cảnh báo một lần.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::ResourceLimits;

const ROOT: &str = "/sys/fs/cgroup";
const CONTROLLER_LEAF: &str = "minehost-controller";
/// `pids` của cgroup đếm cả luồng; server có mod dùng vài trăm luồng.
const PIDS_MAX: u32 = 2048;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(crate) struct Cgroups {
    base: PathBuf,
}

impl Cgroups {
    /// `None` khi không có cgroup v2 ghi được, hoặc thiếu controller `memory`, `pids`.
    pub(crate) fn detect() -> Option<Self> {
        match Self::try_setup() {
            Ok(cgroups) => Some(cgroups),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "không dùng được cgroup v2: server chạy không có giới hạn RAM của MineHost"
                );
                None
            }
        }
    }

    fn try_setup() -> io::Result<Self> {
        let own = own_cgroup()?;
        // Đã chuyển vào lá của controller từ lần trước (nhiều `ProcessRuntime` trong một process).
        let base = if own.file_name().is_some_and(|n| n == CONTROLLER_LEAF) {
            own.parent().map(Path::to_path_buf).unwrap_or(own)
        } else {
            own
        };
        let controllers = fs::read_to_string(base.join("cgroup.controllers"))?;
        let has = |name: &str| controllers.split_whitespace().any(|c| c == name);
        if !has("memory") || !has("pids") {
            return Err(io::Error::other(
                "cgroup hiện tại không có controller memory và pids",
            ));
        }
        let leaf = base.join(CONTROLLER_LEAF);
        create_dir(&leaf)?;
        fs::write(leaf.join("cgroup.procs"), std::process::id().to_string())?;
        fs::write(base.join("cgroup.subtree_control"), "+memory +pids")?;
        tracing::info!(base = %base.display(), "giới hạn RAM của server bằng cgroup v2");
        Ok(Self { base })
    }

    /// Tạo cgroup cho một process sắp spawn.
    pub(crate) fn create(&self, limits: ResourceLimits) -> io::Result<ServerCgroup> {
        let n = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = self.base.join(format!("mh-{}-{n}", std::process::id()));
        create_dir(&dir)?;
        // Tạo trước để lỗi giữa chừng vẫn xóa được thư mục (khi drop).
        let mut group = ServerCgroup { dir, procs: None };
        fs::write(
            group.dir.join("memory.max"),
            limits.memory_bytes.to_string(),
        )?;
        // Có swap thì JVM chậm lại thay vì bị dừng, người dùng chỉ thấy server lag.
        if group.dir.join("memory.swap.max").exists() {
            fs::write(group.dir.join("memory.swap.max"), "0")?;
        }
        fs::write(group.dir.join("pids.max"), PIDS_MAX.to_string())?;
        let procs = OpenOptions::new()
            .write(true)
            .open(group.dir.join("cgroup.procs"))?;
        group.procs = Some(procs);
        Ok(group)
    }
}

/// cgroup của một process. Bị xóa khi drop; lúc đó process đã thoát nên cgroup rỗng.
#[derive(Debug)]
pub(crate) struct ServerCgroup {
    dir: PathBuf,
    /// Mở sẵn trước khi fork: process con ghi `0` vào đây để tự chuyển vào cgroup.
    procs: Option<File>,
}

impl ServerCgroup {
    pub(crate) fn procs_fd(&self) -> Option<RawFd> {
        self.procs.as_ref().map(AsRawFd::as_raw_fd)
    }

    /// Process con đã exec; không cần giữ file nữa.
    pub(crate) fn spawned(&mut self) {
        self.procs = None;
    }

    /// Kernel đã kill ít nhất một process trong cgroup vì vượt `memory.max`.
    pub(crate) fn oom_killed(&self) -> bool {
        fs::read_to_string(self.dir.join("memory.events")).is_ok_and(|text| oom_kills(&text) > 0)
    }
}

impl Drop for ServerCgroup {
    fn drop(&mut self) {
        if let Err(e) = fs::remove_dir(&self.dir) {
            tracing::warn!(dir = %self.dir.display(), error = %e, "không xóa được cgroup của server");
        }
    }
}

fn own_cgroup() -> io::Result<PathBuf> {
    let text = fs::read_to_string("/proc/self/cgroup")?;
    let rel = text
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .ok_or_else(|| io::Error::other("không có cgroup v2 (thiếu dòng `0::`)"))?;
    Ok(Path::new(ROOT).join(rel.trim_start_matches('/')))
}

fn create_dir(dir: &Path) -> io::Result<()> {
    match fs::create_dir(dir) {
        Err(e) if e.kind() != io::ErrorKind::AlreadyExists => Err(e),
        _ => Ok(()),
    }
}

fn oom_kills(memory_events: &str) -> u64 {
    memory_events
        .lines()
        .find_map(|line| line.strip_prefix("oom_kill "))
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_oom_kill_count() {
        let events = "low 0\nhigh 0\nmax 12\noom 1\noom_kill 1\noom_group_kill 0\n";
        assert_eq!(oom_kills(events), 1);
        assert_eq!(oom_kills("oom 0\noom_kill 0\n"), 0);
        assert_eq!(oom_kills(""), 0);
    }
}
