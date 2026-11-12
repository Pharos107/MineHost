# ADR 0004: Chưa dùng SQLite; đĩa là nơi lưu duy nhất

- Trạng thái: đã chốt (2026-10-23 ở spec P1 mục 3.1; giữ nguyên ngày 2026-10-26 ở spec P2 mục 3.1).
  Ghi lại thành ADR ngày 2026-11-11.

## Bối cảnh

Spec gốc đặt SQLite (`sqlx`) trong `mh-store` làm chỉ mục và nơi giữ lịch sử Job, sự kiện, tài khoản.
Spec P1 hoãn sang P2 vì chưa cần; spec P2 hoãn tiếp vì dữ liệu của P2 vẫn chưa cần truy vấn:

- danh sách server lấy bằng cách quét `servers/*/instance.toml`;
- thông tin backup nằm trong file `.json` cạnh từng file backup;
- phiên đăng nhập và chỉ số chỉ giữ trong RAM;
- người chơi đọc từ file của Minecraft.

## Quyết định

P1 và P2 không dùng SQLite. `mh-store` chỉ đọc ghi file. SQLite được xét lại khi có tài khoản và
nhật ký thao tác (P5).

## Hệ quả

- Không có migration, không có bước đối chiếu đĩa với cơ sở dữ liệu, không thêm phụ thuộc nặng.
- Mọi thứ không nằm trên đĩa mất khi controller tắt: Job (giữ 50 cái gần nhất trong RAM), lịch sử chỉ
  số (1 giờ), phiên đăng nhập.
- Liệt kê là quét thư mục. Chấp nhận được với mục tiêu vài chục server và vài chục backup mỗi server.
- Bảng crate trong `AGENTS.md` và spec gốc từng ghi `mh-store` có SQLite; nay đã sửa `AGENTS.md`, còn
  spec gốc được ghi chú ở `docs/architecture/tong-quan.md`.
