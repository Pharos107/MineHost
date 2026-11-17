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
  (ghi nguyên tử) rồi trả về; đồng thời quét lại `registry.json` từ `mods/*.jar`. Lệnh `help` hiện trong console như lệnh người dùng gõ.
- `GET /servers/{id}/commands`: danh sách đã lưu và thời điểm quét; chưa quét thì danh sách rỗng.
- `GET /servers/{id}/commands/registry`: trả danh sách ID vật phẩm, hiệu ứng, sinh vật, phù phép trích từ `assets/*/lang/en_us.json` trong `mods/*.jar` (cache trong `registry.json`, ghi nguyên tử; tự quét một lần nếu chưa có file cache).
- Mỗi lệnh có `name`, `usage` (cắt còn 300 ký tự) và `description` (chỉ có ở Paper, Purpur). Tối đa
  2.000 lệnh. Tên chỉ gồm chữ, số và `_ . : -`; dòng như `[STDOUT]: /home/...` không bị nhận nhầm.
- **Trang Console:**
  - Gợi ý gồm các lệnh vanilla có mô tả tiếng Việt cộng các lệnh đã quét (có nhãn "Mod / Server"). Giữ nguyên dấu `/` ở đầu nếu người dùng đã gõ `/`; gõ `/` hiện ngay danh sách lệnh.
  - Bấm `Tab` (hoặc chọn gợi ý) điền đối số hiện tại và **tự động mở gợi ý cho vị trí kế tiếp** theo cây lệnh (hỗ trợ nhánh và tham số cố định của các lệnh vanilla như `gamemode`, `effect`, `give`, `clear`, `time`, `weather`, `difficulty`, `whitelist`, `xp`, `tp`, `gamerule`, `summon`, `enchant`, `locate`…, cũng như tự tách danh sách `(a|b|c)` và tham số người chơi/vật phẩm/hiệu ứng từ `usage` đã quét của mod/plugin).
  - Gợi ý các biến trong game: tích hợp từ điển Vanilla song ngữ Việt–Anh (hiệu ứng, vật phẩm, sinh vật, phù phép, công trình, quần xã — tìm được bằng cả tiếng Việt không dấu lẫn ID) kết hợp với registry quét từ `mods/*.jar` để không lag trên modpack 300+ mod.
  - Hiện thanh cú pháp trực tiếp ngay trên ô nhập để tô đậm tham số đang gõ, kèm nhãn phân loại gợi ý và dòng phím tắt bên dưới; khung log tự giãn theo chiều cao màn hình để không dư khoảng trống đáy.
  - Nút icon ở cuối hàng lệnh nhanh (cùng một dòng); chú thích của nút ghi số lệnh và giờ quét lần trước. Không tự quét `help`, vì `help` in hàng trăm dòng vào console.

## Tiêu chí chấp nhận

- Đọc đúng mẫu thật của Fabric và Paper (fixture trong `mh-core`), bỏ dòng không phải lệnh; trích đúng key `item.`, `block.`, `effect.`, `entity.`, `enchantment.` từ `en_us.json` của mod (Rust test).
- Quét với `fake-mc`: lưu `commands.json`, đọc lại được; server tắt thì báo `SERVER_NOT_RUNNING` (Rust test).
- Quét `mods/*.jar` ra `registry.json` và trả qua API (Rust test).
- UI gộp gợi ý, bấm quét gọi API, hiện thời điểm quét; giữ `/`, `Tab` chuyển tiếp sang gợi ý đối số tiếp theo, tách được nhánh `(a|b|c)` từ lệnh mod, tìm được hiệu ứng/vật phẩm song ngữ Việt–Anh và đồ mod, hiện thanh cú pháp (Vitest).

## Không làm

- Chạy Data Generator (`--reports`) hay tự cài mod/plugin bridge vào server để dump Brigadier tree lúc runtime.
- Tự quét lại `help` khi đổi mod: người dùng bấm "Quét lại".
