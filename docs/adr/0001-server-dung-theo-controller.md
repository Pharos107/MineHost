# ADR 0001: Server dừng theo controller

- Trạng thái: đã chốt (2026-10-23)

## Bối cảnh

Cần quyết định server Minecraft có tiếp tục chạy khi controller tắt hay không. Các lựa chọn:
dừng theo controller (A), chạy độc lập rồi gắn lại qua `latest.log` + RCON (B), hoặc một shim riêng
cho mỗi server (C).

## Quyết định

Chọn A. Server là process con; controller crash thì OS kill process con (Job Object trên Windows,
`PR_SET_PDEATHSIG` trên Linux). Từ P1, controller chạy nền không có cửa sổ console, có icon khay
hệ thống, và chỉ thoát qua nút "Thoát" an toàn: gửi `stop` tới từng server rồi mới tắt.

## Hệ quả

- Không có process mồ côi, console luôn đầy đủ, lệnh luôn gửi được qua stdin.
- Cập nhật controller sẽ làm gián đoạn người chơi. Nếu cần tránh, thêm chế độ B vào
  `ProcessRuntime` ở P4 mà không đổi API của supervisor.
