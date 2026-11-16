# Spec — Gợi ý lệnh theo mod và plugin của server

## Vấn đề

Ô lệnh ở trang Console chỉ gợi ý 25 lệnh vanilla viết sẵn. Server có mod hoặc plugin thì còn hàng trăm
lệnh khác (Prominence II có 194 lệnh: `/ftbquests`, `/create`, `/ae2`, `/prominent:...`), người dùng
không biết có những lệnh nào và cách gõ ra sao.

## Cách lấy danh sách

Minecraft in danh sách lệnh ra console khi gõ `help`. Đã thử trên server thật (2026-11-16):

- **Fabric, Quilt, Forge, NeoForge, Vanilla:** `help` in mỗi lệnh một dòng `/<tên> <cách gõ>`, gồm cả
  lệnh của mod. Prominence II (Fabric 1.20.1): 194 dòng trong cùng một giây; dòng dài nhất 4.113 ký tự.
- **Paper, Purpur:** `help` là trang trợ giúp của Bukkit, mỗi dòng `/<tên>: <mô tả>`, gồm cả lệnh
  plugin nhưng không có cách gõ. `minecraft:help` in cách gõ như vanilla nhưng không có lệnh plugin.
  MineHost gửi cả hai rồi gộp theo tên. Mô tả "A Mojang provided command." bị bỏ.

## Hành vi

- `POST /servers/{id}/commands/scan`: server phải đang chạy (không thì `SERVER_NOT_RUNNING`). MineHost
  gửi lệnh `help` như trên, đọc các dòng stdout có dạng ở trên trong lúc chờ: dừng khi đã có dòng và
  1,5 giây không có dòng mới, hoặc sau 10 giây. Lưu kết quả vào `commands.json` trong thư mục server
  (ghi nguyên tử) rồi trả về. Lệnh `help` hiện trong console như lệnh người dùng gõ.
- `GET /servers/{id}/commands`: danh sách đã lưu và thời điểm quét; chưa quét thì danh sách rỗng.
- Mỗi lệnh có `name`, `usage` (cắt còn 300 ký tự) và `description` (chỉ có ở Paper, Purpur). Tối đa
  2.000 lệnh. Tên chỉ gồm chữ, số và `_ . : -`; dòng như `[STDOUT]: /home/...` không bị nhận nhầm.
- **Trang Console:** gợi ý gồm 25 lệnh có mô tả tiếng Việt cộng các lệnh đã quét (có nhãn "server").
  Nút "Lấy lệnh của server" trong ô gợi ý; đã quét thì ghi thời điểm và cho quét lại. Không tự quét,
  vì `help` in hàng trăm dòng vào console.

## Tiêu chí chấp nhận

- Đọc đúng mẫu thật của Fabric và Paper (fixture trong `mh-core`), bỏ dòng không phải lệnh (Rust test).
- Quét với `fake-mc`: lưu `commands.json`, đọc lại được; server tắt thì báo `SERVER_NOT_RUNNING` (Rust test).
- UI gộp gợi ý, bấm quét gọi API, hiện thời điểm quét (Vitest).

## Không làm

- Gợi ý theo từng đối số (tên người chơi, vật phẩm): `help` chỉ cho cách gõ ở mức đầu.
- Tự quét lại khi đổi mod: người dùng bấm "Quét lại".
