# Kết quả thử tay P1

Theo [checklist 5 phút](p1-mot-server-chay-duoc.md#11-checklist-thử-tay-cho-tiêu-chí-5-phút) của spec P1.

## Lần chạy chính thức trên Windows mới cài

Chưa làm. Cần một máy hoặc VM Windows 11 mới cài, chưa có Java, mạng ≥ 20 Mbps, và một người chưa
từng dùng MineHost. Dùng file `minehost-v0.2.0-x86_64-pc-windows-msvc.zip` trong bản Release v0.2.0.

| Mục | Kết quả |
| --- | --- |
| Ngày, máy, mạng | |
| Thời gian từ bấm đúp tới vào được từ LAN | |
| Chỗ phải đọc lại hoặc đoán | |
| Lỗi gặp phải | |
| Đạt (dưới 5 phút, không cần trợ giúp) | |

## Thử trên máy phát triển (2026-10-24)

Không thay được lần chạy chính thức (máy đã có công cụ phát triển, người thử biết MineHost), nhưng
cho biết luồng chạy được và thời gian tải:

- Windows 11, Ryzen 7 7840HS, 31 GB RAM. Chạy bản debug từ mã nguồn, thư mục dữ liệu trống.
- Wizard 3 bước với giá trị mặc định (Paper 26.3, RAM đề xuất 7,5 GB, cổng tự chọn 25565).
- Job tạo server: tải Java 25 (56 MB) và Paper, xong trong chưa tới 30 giây; giao diện hiện tiến độ
  từng bước, ví dụ "Tải Java 25 (43/56 MB)", rồi tự chuyển sang trang server.
- Trang server hiện địa chỉ LAN `192.168.1.5:25565` kèm nút chép; bấm "Bật server" thì hiện hộp
  thoại EULA. Phần bật server và vào chơi không thử tay vì phải đồng ý EULA của Mojang; luồng
  này được test tích hợp kiểm tra (dưới đây).

## Test tích hợp trong CI

Ba tiêu chí còn lại của P1 được kiểm tra bằng `crates/minehost/tests/paper.rs` (Paper 1.21.4,
Java 21) trong job `integration`, đều qua trên Windows và Linux:
[lần chạy ngày 2026-10-24](https://github.com/Pharos107/MineHost/actions/runs/40060305402).

| Test | Windows | Linux |
| --- | --- | --- |
| `paper_full_flow` (tạo → EULA → bật → `Done` → `list` → tắt sạch) | qua | qua |
| `killed_controller_leaves_no_orphan_java` | qua | qua |
| `quit_stops_three_servers_cleanly` | qua | qua |
