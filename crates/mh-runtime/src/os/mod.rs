//! Phần riêng của từng OS: process con không sống sót khi controller chết, và giới hạn tài
//! nguyên của từng process (spec P3, mục 3.8). Đây là nơi duy nhất trong workspace được dùng
//! `unsafe`.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

use crate::ResourceLimits;

/// Thứ phải giữ suốt vòng đời luồng spawner. Linux dựa vào chính luồng đó (`PR_SET_PDEATHSIG`).
/// Windows giữ một Job Object chung không giới hạn: luồng spawner dừng (drop `ProcessRuntime`,
/// controller thoát hay bị kill) thì OS kill mọi process trong đó.
#[derive(Debug)]
pub(crate) struct Guard {
    #[cfg(windows)]
    job: windows::Job,
}

/// Giữ tới khi process thoát. Trên Windows là Job Object riêng của process, lồng trong job chung,
/// mang giới hạn tài nguyên và completion port.
#[derive(Debug)]
pub(crate) struct ProcessGuard {
    #[cfg(windows)]
    job: windows::Job,
}

impl Guard {
    pub(crate) fn new() -> std::io::Result<Self> {
        Ok(Self {
            #[cfg(windows)]
            job: windows::Job::new(None)?,
        })
    }

    /// Cấu hình lệnh trước khi spawn.
    pub(crate) fn prepare(&self, cmd: &mut tokio::process::Command) {
        #[cfg(windows)]
        cmd.creation_flags(windows::CREATE_NO_WINDOW);
        #[cfg(target_os = "linux")]
        linux::kill_with_parent(cmd);
        let _ = cmd;
    }

    /// Gắn process vừa spawn vào cơ chế "chết theo controller" và áp giới hạn.
    pub(crate) fn adopt(
        &self,
        child: &tokio::process::Child,
        limits: Option<ResourceLimits>,
    ) -> std::io::Result<ProcessGuard> {
        #[cfg(windows)]
        {
            let job = windows::Job::new(limits)?;
            if let Some(handle) = child.raw_handle() {
                // Gán vào job chung trước: gán tiếp vào job riêng thì job riêng thành job lồng
                // bên trong (Windows 8 trở lên), nên đóng job chung vẫn kill được process.
                self.job.assign(handle)?;
                job.assign(handle)?;
            }
            Ok(ProcessGuard { job })
        }
        #[cfg(not(windows))]
        {
            let _ = (child, limits);
            Ok(ProcessGuard {})
        }
    }
}

impl ProcessGuard {
    /// Gọi sau khi process đã thoát.
    pub(crate) fn memory_limit_hit(&self) -> bool {
        #[cfg(windows)]
        return self.job.memory_limit_hit();
        #[cfg(not(windows))]
        false
    }
}
