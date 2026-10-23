//! Phần riêng của từng OS để process con không sống sót khi controller chết.
//! Đây là nơi duy nhất trong workspace được dùng `unsafe`.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

/// Thứ phải giữ suốt vòng đời luồng spawner. Trên Windows là Job Object.
#[derive(Debug)]
pub(crate) struct Guard {
    #[cfg(windows)]
    job: windows::Job,
}

impl Guard {
    pub(crate) fn new() -> std::io::Result<Self> {
        Ok(Self {
            #[cfg(windows)]
            job: windows::Job::new()?,
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

    /// Gắn process vừa spawn vào cơ chế "chết theo controller".
    pub(crate) fn adopt(&self, child: &tokio::process::Child) -> std::io::Result<()> {
        #[cfg(windows)]
        if let Some(handle) = child.raw_handle() {
            self.job.assign(handle)?;
        }
        let _ = child;
        Ok(())
    }
}
