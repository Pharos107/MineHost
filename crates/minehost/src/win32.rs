//! Vài lời gọi Win32 mà binary cần: gắn vào console của terminal cha, và vòng lặp message
//! cho icon khay trên luồng chính (spec P1, mục 5.9; ADR 0002).

// Gọi Win32 API bắt buộc phải dùng `unsafe`; phần này được giữ nhỏ và mỗi khối có giải thích.
#![allow(unsafe_code)]

use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PostThreadMessageW, TranslateMessage, WM_APP, WM_QUIT,
};

/// Message riêng để luồng khác đánh thức luồng khay (đổi tooltip, đổi chữ menu).
const WM_WAKE: u32 = WM_APP + 1;

/// Bản release không có cửa sổ console. Chạy từ terminal (ví dụ `minehost paths`) thì gắn vào
/// console của terminal để lệnh vẫn in ra được; bấm đúp từ Explorer thì không có gì để gắn.
pub fn attach_parent_console() {
    // SAFETY: không có con trỏ nào; thất bại (không có console cha) chỉ trả về 0 và bị bỏ qua.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

pub fn current_thread_id() -> u32 {
    // SAFETY: hàm không có tham số và luôn thành công.
    unsafe { GetCurrentThreadId() }
}

/// Đánh thức `run_message_loop` trên luồng `thread`. Luồng đã thoát thì lời gọi thất bại, bỏ qua.
pub fn post_wake(thread: u32) {
    // SAFETY: chỉ truyền số; Windows tự kiểm tra `thread` có hàng đợi message hay không.
    unsafe {
        PostThreadMessageW(thread, WM_WAKE, 0, 0);
    }
}

/// Cho `run_message_loop` trên luồng `thread` kết thúc.
pub fn post_quit(thread: u32) {
    // SAFETY: như `post_wake`.
    unsafe {
        PostThreadMessageW(thread, WM_QUIT, 0, 0);
    }
}

/// Chạy vòng lặp message của luồng hiện tại tới khi nhận `WM_QUIT`. Menu và icon khay chỉ hoạt
/// động khi luồng tạo ra chúng còn xử lý message. Mỗi lần `post_wake` thì gọi `on_wake`.
pub fn run_message_loop(mut on_wake: impl FnMut()) {
    let mut msg = MSG::default();
    loop {
        // SAFETY: `msg` là biến cục bộ hợp lệ để Windows ghi vào; hwnd null nghĩa là nhận mọi
        // message của luồng này, kể cả message gửi bằng `PostThreadMessageW`.
        let result = unsafe { GetMessageW(&raw mut msg, std::ptr::null_mut(), 0, 0) };
        // 0: WM_QUIT; -1: lỗi (hwnd sai), không thể xảy ra với hwnd null nhưng vẫn dừng cho an toàn.
        if result == 0 || result == -1 {
            return;
        }
        if msg.hwnd.is_null() && msg.message == WM_WAKE {
            on_wake();
            continue;
        }
        // SAFETY: `msg` vừa được `GetMessageW` điền đầy đủ.
        unsafe {
            TranslateMessage(&raw const msg);
            DispatchMessageW(&raw const msg);
        }
    }
}
