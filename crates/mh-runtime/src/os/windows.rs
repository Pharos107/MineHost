//! Job Object của Windows: process con chết theo controller.
//!
//! Mọi process server được gán vào một Job Object có `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
//! Handle của job chỉ do luồng spawner giữ; luồng đó (hoặc cả controller) kết thúc thì OS
//! đóng handle và kill mọi process trong job, kể cả khi controller bị kill đột ngột.

// Gọi Win32 API bắt buộc phải dùng `unsafe`; phần này được giữ nhỏ và mỗi khối có giải thích.
#![allow(unsafe_code)]

use std::io;
use std::os::windows::io::RawHandle;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};

/// Không mở cửa sổ console cho Java khi controller chạy ẩn.
pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug)]
pub(crate) struct Job(HANDLE);

impl Job {
    pub(crate) fn new() -> io::Result<Self> {
        // SAFETY: cả hai tham số được phép null (thuộc tính bảo mật mặc định, job không tên).
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = Self(handle);
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let size = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
            .map_err(io::Error::other)?;
        // SAFETY: `job.0` là handle hợp lệ vừa tạo; con trỏ trỏ tới `info` còn sống trong suốt
        // lời gọi, đúng kiểu và đúng kích thước mà `JobObjectExtendedLimitInformation` yêu cầu.
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
        Ok(job)
    }

    /// Gán process vào job. `process` phải là handle của một process còn đang sống.
    pub(crate) fn assign(&self, process: RawHandle) -> io::Result<()> {
        // SAFETY: `self.0` là handle job hợp lệ cho tới khi `Drop`; `process` do tokio giữ và còn
        // mở trong suốt lời gọi, vì `Child` chưa bị drop.
        let ok = unsafe { AssignProcessToJobObject(self.0, process.cast()) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: handle do `CreateJobObjectW` trả về và chỉ được đóng đúng một lần ở đây.
        unsafe {
            CloseHandle(self.0);
        }
    }
}
