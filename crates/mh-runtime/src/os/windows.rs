//! Job Object của Windows: process con chết theo controller và bị giới hạn tài nguyên.
//!
//! Mỗi process có một Job Object riêng với `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. Handle của job
//! do task theo dõi process giữ; controller kết thúc (kể cả bị kill đột ngột) thì OS đóng handle
//! và kill mọi process trong job. Có `ResourceLimits` thì job còn giới hạn bộ nhớ commit và số
//! process (spec P3, mục 3.8), kèm một completion port để biết job đã chạm giới hạn bộ nhớ.

// Gọi Win32 API bắt buộc phải dùng `unsafe`; phần này được giữ nhỏ và mỗi khối có giải thích.
#![allow(unsafe_code)]

use std::io;
use std::os::windows::io::RawHandle;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::IO::{
    CreateIoCompletionPort, GetQueuedCompletionStatus, OVERLAPPED,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
    JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_ASSOCIATE_COMPLETION_PORT, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectAssociateCompletionPortInformation, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};

use crate::ResourceLimits;

/// Không mở cửa sổ console cho Java khi controller chạy ẩn.
pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `JOB_OBJECT_MSG_JOB_MEMORY_LIMIT` trong `winnt.h`. Ghi tay để không kéo cả feature
/// `Win32_System_SystemServices` của `windows-sys` chỉ vì một hằng số.
const MSG_JOB_MEMORY_LIMIT: u32 = 10;

/// Handle Win32 đóng đúng một lần khi drop.
#[derive(Debug)]
struct Owned(HANDLE);

// SAFETY: handle kernel của Win32 không gắn với luồng tạo ra nó; dùng và đóng từ luồng khác đều
// hợp lệ. `Owned` không `Sync` và không cho chép handle, nên chỉ một luồng giữ nó tại một lúc.
unsafe impl Send for Owned {}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: handle do Win32 trả về, khác null, và chỉ được đóng đúng một lần ở đây.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[derive(Debug)]
pub(crate) struct Job {
    // Thứ tự trường là thứ tự drop: đóng port trước, job sau.
    port: Option<Owned>,
    job: Owned,
}

impl Job {
    pub(crate) fn new(limits: Option<ResourceLimits>) -> io::Result<Self> {
        // SAFETY: cả hai tham số được phép null (thuộc tính bảo mật mặc định, job không tên).
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = Owned(handle);
        set_limits(&job, limits)?;
        let port = match limits {
            Some(_) => Some(watch_port(&job)?),
            None => None,
        };
        Ok(Self { port, job })
    }

    /// Gán process vào job. `process` phải là handle của một process còn đang sống.
    pub(crate) fn assign(&self, process: RawHandle) -> io::Result<()> {
        // SAFETY: `self.job.0` là handle job hợp lệ cho tới khi drop; `process` do tokio giữ và
        // còn mở trong suốt lời gọi, vì `Child` chưa bị drop.
        let ok = unsafe { AssignProcessToJobObject(self.job.0, process.cast()) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Đọc hết thông báo đang chờ trong completion port, không chờ thêm.
    pub(crate) fn memory_limit_hit(&self) -> bool {
        let Some(port) = &self.port else {
            return false;
        };
        let mut hit = false;
        loop {
            let mut message = 0u32;
            let mut key = 0usize;
            let mut overlapped: *mut OVERLAPPED = std::ptr::null_mut();
            // SAFETY: `port.0` là completion port hợp lệ; ba con trỏ trỏ tới biến cục bộ còn sống
            // trong suốt lời gọi. Thời gian chờ 0 nên lời gọi không chặn.
            let ok = unsafe {
                GetQueuedCompletionStatus(port.0, &mut message, &mut key, &mut overlapped, 0)
            };
            if ok == 0 {
                return hit;
            }
            hit |= message == MSG_JOB_MEMORY_LIMIT;
        }
    }
}

fn set_limits(job: &Owned, limits: Option<ResourceLimits>) -> io::Result<()> {
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if let Some(limits) = limits {
        info.BasicLimitInformation.LimitFlags |=
            JOB_OBJECT_LIMIT_JOB_MEMORY | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        // Bản 64-bit: `usize` đủ chứa; quá thì kẹp ở mức lớn nhất thay vì bỏ giới hạn.
        info.JobMemoryLimit = usize::try_from(limits.memory_bytes).unwrap_or(usize::MAX);
        info.BasicLimitInformation.ActiveProcessLimit = limits.max_processes;
    }
    let size = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
        .map_err(io::Error::other)?;
    // SAFETY: `job.0` là handle hợp lệ vừa tạo; con trỏ trỏ tới `info` còn sống trong suốt lời
    // gọi, đúng kiểu và đúng kích thước mà `JobObjectExtendedLimitInformation` yêu cầu.
    let ok = unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&raw const info).cast(),
            size,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Completion port nhận thông báo của job, trong đó có "đã chạm giới hạn bộ nhớ".
fn watch_port(job: &Owned) -> io::Result<Owned> {
    // SAFETY: `INVALID_HANDLE_VALUE` với port null nghĩa là tạo port mới không gắn file nào.
    let handle =
        unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, std::ptr::null_mut(), 0, 1) };
    if handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    let port = Owned(handle);
    let info = JOBOBJECT_ASSOCIATE_COMPLETION_PORT {
        CompletionKey: std::ptr::null_mut(),
        CompletionPort: port.0,
    };
    let size = u32::try_from(size_of::<JOBOBJECT_ASSOCIATE_COMPLETION_PORT>())
        .map_err(io::Error::other)?;
    // SAFETY: `job.0` và `port.0` là handle hợp lệ; `info` còn sống trong suốt lời gọi, đúng kiểu
    // và kích thước của `JobObjectAssociateCompletionPortInformation`.
    let ok = unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectAssociateCompletionPortInformation,
            (&raw const info).cast(),
            size,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(port)
}
