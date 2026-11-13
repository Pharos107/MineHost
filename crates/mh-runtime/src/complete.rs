//! Chạy một process tới khi thoát, dùng cho installer của loader (spec P3, mục 3.1; ADR 0006).
//!
//! Installer chạy qua cùng luồng spawner với server, nên có cùng Job Object trên Windows và
//! `PR_SET_PDEATHSIG` trên Linux: controller chết thì installer chết theo.

use std::future::Future;
use std::time::Duration;

use crate::{OutputLine, ProcessRuntime, ProcessSpec, RuntimeError};

/// Kết quả của [`ProcessRuntime::run_to_completion`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    /// Process tự thoát. `code` là `None` khi bị kill bằng tín hiệu (Linux).
    Exited { code: Option<i32> },
    /// Quá thời hạn, đã kill.
    TimedOut,
    /// Người gọi hủy, đã kill.
    Cancelled,
}

impl Completion {
    pub const fn success(self) -> bool {
        matches!(self, Self::Exited { code: Some(0) })
    }
}

impl ProcessRuntime {
    /// Chạy `spec` tới khi thoát, quá `timeout` hoặc `cancelled` xong. Mọi dòng log đi qua
    /// `on_line`, kể cả các dòng còn trong pipe sau khi process thoát.
    ///
    /// stdin bị đóng ngay: installer không được chờ người dùng gõ gì.
    pub async fn run_to_completion(
        &self,
        spec: ProcessSpec,
        timeout: Duration,
        cancelled: impl Future<Output = ()> + Send,
        mut on_line: impl FnMut(OutputLine) + Send,
    ) -> Result<Completion, RuntimeError> {
        let running = self.spawn(spec).await?;
        // Giữ kênh kill, bỏ kênh stdin: task ghi stdin kết thúc và đóng pipe.
        let kill = running.control.kill.clone();
        drop(running.control);
        let mut output = running.output;
        let mut exit = running.exit;
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline, cancelled);
        let mut stopped_by = None;
        let mut output_open = true;
        let code = loop {
            tokio::select! {
                // Kênh đóng trước khi có `exit` thì thôi đọc, để vòng lặp không quay tròn.
                line = output.recv(), if output_open => match line {
                    Some(line) => on_line(line),
                    None => output_open = false,
                },
                result = &mut exit => break result.ok().and_then(|e| e.code),
                () = &mut deadline, if stopped_by.is_none() => {
                    stopped_by = Some(Completion::TimedOut);
                    let _ = kill.try_send(());
                }
                () = &mut cancelled, if stopped_by.is_none() => {
                    stopped_by = Some(Completion::Cancelled);
                    let _ = kill.try_send(());
                }
            }
        };
        // `exit` chỉ được gửi sau khi đã đọc xong pipe, nên phần còn lại đã nằm trong kênh.
        while let Ok(line) = output.try_recv() {
            on_line(line);
        }
        Ok(stopped_by.unwrap_or(Completion::Exited { code }))
    }
}
