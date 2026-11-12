# ADR 0005: Điều khiển server qua console, không dùng RCON

- Trạng thái: đã chốt (2026-10-26 ở spec P2 mục 3.2). Ghi lại thành ADR ngày 2026-11-11.

## Bối cảnh

P2 cần gửi lệnh cho server đang chạy: quản lý người chơi (`whitelist`, `kick`, `ban`, `attribute`),
sao lưu (`save-off`, `save-all flush`, `save-on`), báo trước khi khởi động lại (`say`). Có hai đường:
RCON của Minecraft, hoặc stdin của process mà MineHost đã giữ sẵn (ADR 0001).

## Quyết định

Mọi lệnh đi qua supervisor như một dòng console. Không bật RCON. Kết quả của lệnh đọc từ log:

- `send_and_wait(line, pattern, timeout)` gửi lệnh rồi chờ dòng log khớp mẫu (ví dụ `Saved the game`);
- danh sách người đang online lấy từ dòng `joined the game` / `left the game`, cộng một lần `list` khi
  server sang `Running` để đồng bộ lại.

## Hệ quả

- Không mở thêm cổng, không phải sinh và giữ mật khẩu RCON, không cần sửa `server.properties`.
- Phụ thuộc vào định dạng log. Log khác nhau giữa các bản Minecraft và loader, và plugin có thể đổi
  nội dung thông báo. Giảm rủi ro bằng fixture log của nhiều phiên bản và lần `list` đồng bộ.
- Nội dung do người chơi tạo ra (tên, chat) đi vào cùng luồng log. Để chat không giả được sự kiện, bộ
  phân tích đòi toàn bộ nội dung dòng (sau tiền tố log) phải đúng mẫu `<tên hợp lệ> joined the game`;
  dòng chat luôn bắt đầu bằng `<người gửi>` nên không khớp.
- Lệnh không có giá trị trả về có cấu trúc. Tính năng nào cần dữ liệu có cấu trúc (ví dụ TPS, MSPT)
  phải tự phân tích văn bản hoặc dùng đường khác.
- Ràng buộc vào ADR 0001: nếu sau này server chạy tách khỏi controller thì stdin không còn, và phải
  quay lại RCON.
