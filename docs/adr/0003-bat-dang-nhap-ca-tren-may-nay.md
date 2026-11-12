# ADR 0003: Bắt đăng nhập cả trên máy này

- Trạng thái: đã chốt (2026-10-27)

## Bối cảnh

MineHost phục vụ giao diện trên máy đang chạy server và, từ P2, có thể mở cho máy
khác trong mạng LAN. Khi người dùng đã đặt mật khẩu admin, cần quyết định request
từ chính máy này (`127.0.0.1`) có phải đăng nhập hay không. Giao diện và API dùng
cùng một cổng.

## Quyết định

Đã đặt mật khẩu thì mọi API, kể cả từ `127.0.0.1`, phải đăng nhập (spec P2, mục 3.3).
Chưa đặt mật khẩu thì không hỏi gì, giống P1.

Quên mật khẩu thì đặt lại bằng một trong hai cách. Cả hai đều cần ngồi trước máy:

- Lệnh `minehost reset-password` (Windows và Linux) xóa mật khẩu và tắt LAN. Lệnh
  chạy được cả khi MineHost đang chạy: controller đọc lại `auth.toml` khi thời điểm
  sửa file đổi.
- Trên Windows, menu khay "Đặt lại mật khẩu". Bấm lần đầu thì chữ đổi thành
  "Bấm lần nữa để xóa mật khẩu" trong 10 giây. Khay không có hộp thoại (ADR 0002).

## Hệ quả

- Quên mật khẩu thì dùng `reset-password` hoặc menu khay, không khóa mất máy.
- Phiên đăng nhập chỉ nằm trong RAM. Tắt MineHost là hết phiên. Không có phiên
  trên đĩa để lấy trộm.
- Bật LAN khi chưa có TLS thì mật khẩu đi trên mạng nội bộ dạng không mã hóa
  (mục 3.4). LAN tắt mặc định và chỉ bật được sau khi đã có mật khẩu.

## Phương án đã cân nhắc

Phương án không bắt đăng nhập ở `127.0.0.1` bị loại. Ai mở được trình duyệt trên
máy sẽ dùng được toàn bộ API dù chủ máy đã đặt mật khẩu. Người dùng chọn bắt
đăng nhập cả trên máy này.
