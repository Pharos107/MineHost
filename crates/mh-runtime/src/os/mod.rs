//! Phần riêng của từng OS: process con không sống sót khi controller chết, và giới hạn tài
//! nguyên của từng process (spec P3, mục 3.8). Đây là nơi duy nhất trong workspace được dùng
//! `unsafe`.

#[cfg(target_os = "linux")]
mod cgroup;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

use crate::ResourceLimits;

/// Thứ phải giữ suốt vòng đời luồng spawner. Linux giữ thông tin cgroup (nếu dùng được) và dựa
/// vào chính luồng đó cho `PR_SET_PDEATHSIG`. Windows giữ một Job Object chung không giới hạn:
/// luồng spawner dừng (drop `ProcessRuntime`, controller thoát hay bị kill) thì OS kill mọi
/// process trong đó.
#[derive(Debug)]
pub(crate) struct Guard {
    #[cfg(windows)]
    job: windows::Job,
    #[cfg(target_os = "linux")]
    cgroups: Option<cgroup::Cgroups>,
}

/// Chuẩn bị cho một lần spawn, từ `prepare` tới `adopt`.
#[derive(Debug)]
pub(crate) struct Pending {
    limits: Option<ResourceLimits>,
    #[cfg(target_os = "linux")]
    cgroup: Option<cgroup::ServerCgroup>,
}

/// Giữ tới khi process thoát. Windows: Job Object riêng của process, lồng trong job chung, mang
/// giới hạn tài nguyên và completion port. Linux: cgroup của process, xóa khi drop.
#[derive(Debug)]
pub(crate) struct ProcessGuard {
    #[cfg(windows)]
    job: windows::Job,
    #[cfg(target_os = "linux")]
    cgroup: Option<cgroup::ServerCgroup>,
}

impl Guard {
    pub(crate) fn new() -> std::io::Result<Self> {
        Ok(Self {
            #[cfg(windows)]
            job: windows::Job::new(None)?,
            #[cfg(target_os = "linux")]
            cgroups: cgroup::Cgroups::detect(),
        })
    }

    /// `ResourceLimits` có được OS áp thật hay không.
    #[cfg(target_os = "linux")]
    pub(crate) fn limits_enforced(&self) -> bool {
        self.cgroups.is_some()
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn limits_enforced(&self) -> bool {
        cfg!(windows)
    }

    /// Cấu hình lệnh trước khi spawn.
    pub(crate) fn prepare(
        &self,
        cmd: &mut tokio::process::Command,
        limits: Option<ResourceLimits>,
    ) -> Pending {
        #[cfg(windows)]
        cmd.creation_flags(windows::CREATE_NO_WINDOW);
        #[cfg(target_os = "linux")]
        let cgroup = {
            linux::kill_with_parent(cmd);
            let cgroup = self.linux_cgroup(limits);
            if let Some(fd) = cgroup.as_ref().and_then(cgroup::ServerCgroup::procs_fd) {
                linux::join_cgroup(cmd, fd);
            }
            cgroup
        };
        let _ = cmd;
        Pending {
            limits,
            #[cfg(target_os = "linux")]
            cgroup,
        }
    }

    #[cfg(target_os = "linux")]
    fn linux_cgroup(&self, limits: Option<ResourceLimits>) -> Option<cgroup::ServerCgroup> {
        let (cgroups, limits) = (self.cgroups.as_ref()?, limits?);
        // Không tạo được cgroup cho riêng lần này thì vẫn chạy, như máy không có cgroup.
        cgroups
            .create(limits)
            .inspect_err(|e| tracing::warn!(error = %e, "không tạo được cgroup cho server"))
            .ok()
    }

    /// Gắn process vừa spawn vào cơ chế "chết theo controller" và áp giới hạn.
    pub(crate) fn adopt(
        &self,
        child: &tokio::process::Child,
        pending: Pending,
    ) -> std::io::Result<ProcessGuard> {
        #[cfg(windows)]
        {
            let job = windows::Job::new(pending.limits)?;
            if let Some(handle) = child.raw_handle() {
                // Gán vào job chung trước: gán tiếp vào job riêng thì job riêng thành job lồng
                // bên trong (Windows 8 trở lên), nên đóng job chung vẫn kill được process.
                self.job.assign(handle)?;
                job.assign(handle)?;
            }
            Ok(ProcessGuard { job })
        }
        #[cfg(target_os = "linux")]
        {
            let _ = (child, pending.limits);
            let mut cgroup = pending.cgroup;
            if let Some(cgroup) = cgroup.as_mut() {
                cgroup.spawned();
            }
            Ok(ProcessGuard { cgroup })
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = (child, pending);
            Ok(ProcessGuard {})
        }
    }
}

impl ProcessGuard {
    /// Gọi sau khi process đã thoát.
    #[cfg(windows)]
    pub(crate) fn memory_limit_hit(&self) -> bool {
        self.job.memory_limit_hit()
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn memory_limit_hit(&self) -> bool {
        self.cgroup
            .as_ref()
            .is_some_and(cgroup::ServerCgroup::oom_killed)
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    pub(crate) fn memory_limit_hit(&self) -> bool {
        false
    }
}
