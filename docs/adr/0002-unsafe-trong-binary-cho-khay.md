# ADR 0002: Cho phép `unsafe` trong binary `minehost` cho khay và console

- Trạng thái: đã chốt (2026-10-24)

## Bối cảnh

Spec P1 mục 5.9 yêu cầu bản release trên Windows không có cửa sổ console
(`windows_subsystem = "windows"`), gọi `AttachConsole` khi chạy từ terminal, và có icon khay chạy
vòng lặp sự kiện trên luồng chính. Cả `AttachConsole` lẫn vòng lặp message (`GetMessageW`,
`DispatchMessageW`, `PostThreadMessageW`) đều là Win32 API, chỉ gọi được qua `unsafe`. Trước đây
`mh-runtime` là crate duy nhất mở `unsafe_code`.

Các lựa chọn khác đã xem:

- Dùng `tao` hoặc `winit` để có vòng lặp sự kiện an toàn: kéo theo cả một thư viện cửa sổ chỉ để
  giữ một icon khay.
- Đưa các hàm này vào `mh-runtime` hoặc `mh-platform`: sai trách nhiệm của crate (chạy process,
  thư mục theo OS) và vẫn là `unsafe`, chỉ đổi chỗ.

## Quyết định

Mở `unsafe_code` riêng cho module `crates/minehost/src/win32.rs` (chỉ biên dịch trên Windows),
giống cách `mh-runtime` làm với `os/windows.rs` và `os/linux.rs`: `#![allow(unsafe_code)]` ở đầu
module, mỗi khối `unsafe` có comment `// SAFETY:`. Phần còn lại của binary vẫn bị cấm `unsafe`.

## Hệ quả

- Không thêm thư viện cửa sổ; khay dùng `tray-icon` (chỉ trên Windows, tắt feature mặc định).
- Thêm hai feature của `windows-sys`: `Win32_System_Console`, `Win32_UI_WindowsAndMessaging`.
- Ở khay không có hộp thoại hỏi lại khi Thoát. Thay vào đó chữ trên menu ghi rõ
  "Thoát (tắt an toàn N server)"; thoát luôn đi qua `shutdown_all`, không server nào bị kill.
