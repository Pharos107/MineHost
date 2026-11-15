//! `PR_SET_PDEATHSIG` trên Linux: process con nhận `SIGKILL` khi controller chết.
//!
//! Tín hiệu này gắn với **luồng** đã fork chứ không phải cả process. Vì vậy mọi lần spawn phải
//! đi qua luồng spawner sống suốt vòng đời controller (xem `spawner.rs`), không bao giờ từ
//! `spawn_blocking` hay một luồng tokio có thể tự thoát.

// `pre_exec` và `prctl` bắt buộc phải dùng `unsafe`; phần này được giữ nhỏ.
#![allow(unsafe_code)]

use std::io;
use std::os::fd::RawFd;

pub(crate) fn kill_with_parent(cmd: &mut tokio::process::Command) {
    // Lấy trước khi fork: nếu controller đã chết trước khi `prctl` kịp chạy thì process con
    // có cha mới, và phải tự hủy thay vì chạy mồ côi.
    let parent = std::process::id();
    // SAFETY: closure chạy giữa fork và exec nên chỉ được gọi hàm async-signal-safe. `prctl` và
    // `getppid` đều an toàn ở đây; closure không cấp phát bộ nhớ, không khóa mutex, và lỗi trả
    // về chỉ chứa mã errno.
    unsafe {
        cmd.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(io::Error::last_os_error());
            }
            if u32::try_from(libc::getppid()).ok() != Some(parent) {
                return Err(io::Error::from_raw_os_error(libc::ESRCH));
            }
            Ok(())
        });
    }
}

/// Process con tự chuyển vào cgroup của nó trước khi exec, để Java chưa chạy dòng nào đã bị
/// giới hạn. `procs_fd` là `cgroup.procs` đã mở sẵn ở process cha.
pub(crate) fn join_cgroup(cmd: &mut tokio::process::Command, procs_fd: RawFd) {
    // SAFETY: closure chạy giữa fork và exec nên chỉ được gọi hàm async-signal-safe. `write` là
    // syscall an toàn ở đây; dữ liệu là chuỗi tĩnh, không cấp phát. fd do process cha giữ mở cho
    // tới khi spawn xong. Ghi `0` vào `cgroup.procs` nghĩa là chuyển chính process đang ghi.
    unsafe {
        cmd.pre_exec(move || {
            let written = libc::write(procs_fd, b"0".as_ptr().cast(), 1);
            if written != 1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}
