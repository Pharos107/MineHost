//! Vài lời gọi Win32 mà binary cần: gắn vào console của terminal cha, vòng lặp message cho icon
//! khay trên luồng chính, và cửa sổ ẩn nhận báo tắt máy (spec P1, mục 5.9; ADR 0002).

// Gọi Win32 API bắt buộc phải dùng `unsafe`; phần này được giữ nhỏ và mỗi khối có giải thích.
#![allow(unsafe_code)]

use std::sync::OnceLock;
use std::time::Duration;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Shutdown::{ShutdownBlockReasonCreate, ShutdownBlockReasonDestroy};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG, PostThreadMessageW,
    RegisterClassW, TranslateMessage, WM_APP, WM_ENDSESSION, WM_QUERYENDSESSION, WM_QUIT,
    WNDCLASSW, WS_OVERLAPPED,
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

/// Chữ Windows hiện ở màn hình "ứng dụng đang chặn tắt máy".
const BLOCK_REASON: &str = "MineHost đang lưu world và tắt các server";
/// Giữ màn hình đó tối đa chừng này. Controller xong sớm hơn thì process thoát, luồng này dừng theo.
const SESSION_END_WAIT: Duration = Duration::from_secs(10 * 60);

static ON_SESSION_END: OnceLock<fn()> = OnceLock::new();
static BLOCK_REASON_W: OnceLock<Vec<u16>> = OnceLock::new();

/// Tạo cửa sổ ẩn trên luồng riêng để nhận `WM_QUERYENDSESSION` / `WM_ENDSESSION`.
///
/// Bản release chạy nền không có console, nên chỉ cửa sổ cấp cao nhất (ẩn cũng được, message-only
/// thì không) mới biết máy sắp tắt hay người dùng đăng xuất. Thiếu nó, Windows kết thúc controller
/// và Job Object kill mọi `java.exe` khi chưa kịp lưu world. `on_end` được gọi một lần khi phiên
/// thật sự kết thúc; sau đó cửa sổ chặn tắt máy tới khi process thoát.
pub fn watch_session_end(on_end: fn()) {
    let _ = ON_SESSION_END.set(on_end);
    let spawned = std::thread::Builder::new()
        .name("mh-session".to_owned())
        .spawn(|| {
            if !create_session_window() {
                tracing::warn!("không tạo được cửa sổ nhận báo tắt máy");
                return;
            }
            run_message_loop(|| {});
        });
    if let Err(err) = spawned {
        tracing::warn!(%err, "không tạo được luồng nhận báo tắt máy");
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn create_session_window() -> bool {
    let class = wide("MineHostSession");
    BLOCK_REASON_W.get_or_init(|| wide(BLOCK_REASON));
    // SAFETY: null nghĩa là module của chính process; hàm không ghi gì qua con trỏ.
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    let wc = WNDCLASSW {
        lpfnWndProc: Some(session_proc),
        hInstance: instance,
        lpszClassName: class.as_ptr(),
        ..Default::default()
    };
    // SAFETY: `wc` hợp lệ trong suốt lời gọi; `class` sống tới hết hàm, và Windows chép tên lớp khi
    // đăng ký nên cửa sổ tạo sau đó không cần chuỗi này nữa.
    if unsafe { RegisterClassW(&raw const wc) } == 0 {
        return false;
    }
    // SAFETY: tên lớp vừa đăng ký, mọi con trỏ khác null. Không có cha nên là cửa sổ cấp cao nhất;
    // không gọi `ShowWindow` nên nó không bao giờ hiện ra.
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    !hwnd.is_null()
}

/// Windows gọi trên luồng `mh-session`, trong `DispatchMessageW`.
unsafe extern "system" fn session_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_QUERYENDSESSION => {
            // Đăng ký lý do để Windows chờ, thay vì kết thúc process sau vài giây. Chưa dừng server:
            // ứng dụng khác còn có thể hủy lần tắt máy này.
            if let Some(reason) = BLOCK_REASON_W.get() {
                // SAFETY: `hwnd` là cửa sổ của luồng này; `reason` kết thúc bằng 0 và sống mãi.
                unsafe { ShutdownBlockReasonCreate(hwnd, reason.as_ptr()) };
            }
            1
        }
        WM_ENDSESSION => {
            if wparam != 0 {
                tracing::info!("Windows sắp tắt hoặc đăng xuất, dừng an toàn các server");
                if let Some(on_end) = ON_SESSION_END.get() {
                    on_end();
                }
                // Trả về là Windows được phép kết thúc process. Chờ ở đây để controller gửi `stop`
                // và đợi từng server lưu xong; controller thoát thì cả process dừng.
                std::thread::sleep(SESSION_END_WAIT);
            }
            // SAFETY: như trên; gỡ lý do chưa đăng ký cũng chỉ trả lỗi.
            unsafe { ShutdownBlockReasonDestroy(hwnd) };
            0
        }
        // SAFETY: chuyển nguyên tham số Windows vừa đưa.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
