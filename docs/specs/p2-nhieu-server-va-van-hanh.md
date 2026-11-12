# Spec P2 — Nhiều server và vận hành hằng ngày

- Trạng thái: đã duyệt, đang làm. Tiến độ từng PR: [kế hoạch PR](p2-ke-hoach-pr.md#tiến-độ)
  (cập nhật 2026-11-12). Vấn đề mở sau lần rà soát: mục 12.
- Ngày: 2026-10-26
- Spec gốc: "MineHost v2 — Spec kiến trúc (Rust)", phần Lộ trình P2
- Kế hoạch PR: [p2-ke-hoach-pr.md](p2-ke-hoach-pr.md)
- Tham khảo: các trang của dự án cũ mc-controller (`Server MC Host/internal/web/pages/`)

## 1. Mục tiêu

Người dùng chạy được nhiều server và tự lo mọi việc hằng ngày ngay trong MineHost: xem server đang
ra sao, sửa file, quản lý người chơi, sao lưu và khôi phục, cho máy khác trong LAN mở giao diện.
Không phải mở thư mục server bằng tay hay gõ lệnh console để làm những việc này.

Giao diện làm trước: các trang giống dự án cũ (mục 6), mỗi trang đi kèm backend thật.

### Tiêu chí hoàn thành

- [ ] 10 server Paper chạy song song liên tục 72 giờ trên một máy Linux. Bộ nhớ controller sau
      giờ thứ nhất không tăng quá 20%, không server nào bị kill ngoài ý muốn.
- [ ] Test tự động: tạo world, đặt một khối, backup, phá khối, khôi phục, bật lại server thì khối còn đó.
- [ ] Test fuzz `SafeRoot::join` và bộ giải nén backup chạy 30 phút không tìm ra lỗi thoát thư mục.
- [ ] Đã đặt mật khẩu thì mọi API (kể cả từ `127.0.0.1`) trả `AUTH_REQUIRED` khi chưa đăng nhập.
      Bật LAN thì một máy khác trong LAN đăng nhập và điều khiển được server.
- [ ] Mỗi trang ở mục 6 có ít nhất một test Playwright chạy với API thật.

### Không làm trong P2

Mod loader, mod, plugin, modpack (P3); Docker, playit.gg, UPnP, kiểm tra cổng từ Internet,
tự cập nhật (P4); nhiều tài khoản, vai trò, API token, nhật ký thao tác (P5); TLS cho giao diện
qua LAN (P4); cấm theo IP; TPS, MSPT, chunk, số thực thể; lịch sử chỉ số dài hơn 1 giờ; tải cả
thư mục dưới dạng zip; tô màu cú pháp trong trình sửa file.

## 2. Bố cục và điều hướng

Thanh bên trái thay cho thanh trên và các tab của P1, giống dự án cũ.

| Ngữ cảnh | Mục trên thanh bên | Đường dẫn |
| --- | --- | --- |
| Chung | Trang chủ | `/` |
| | Máy chủ | `/servers` |
| | Tác vụ | `/jobs` |
| | Cài đặt | `/settings` |
| Trong một server | ← Tất cả máy chủ | `/servers` |
| | Tổng quan | `/servers/:id` |
| | Console | `/servers/:id/console` |
| | Tệp | `/servers/:id/files`, sửa tệp ở `/servers/:id/files/edit?path=` |
| | Người chơi | `/servers/:id/players` |
| | Sao lưu | `/servers/:id/backups` |
| | Cài đặt game | `/servers/:id/game` |
| | Cài đặt server | `/servers/:id/settings` |
| Không có thanh bên | Đăng nhập | `/login` |
| | Tạo máy chủ | `/new` (giữ wizard của P1) |

- Đầu mỗi trang server giữ phần của P1: tên, trạng thái, địa chỉ LAN có nút chép, các nút Bật / Tắt /
  Khởi động lại, menu "…" có Buộc dừng.
- Chân thanh bên: đổi ngôn ngữ, nút Thoát (P1), nút Đăng xuất khi đã đặt mật khẩu.
- Màn hình hẹp hơn 768 px: thanh bên thu thành nút menu ở góc trên, mở ra dạng ngăn kéo.
- Đường dẫn cũ `/servers/:id` của P1 vẫn mở được; trang đó giờ là Tổng quan.

## 3. Quyết định riêng của P2

### 3.1 Vẫn chưa dùng SQLite

P2 chưa có dữ liệu cần truy vấn hay giữ lâu trong cơ sở dữ liệu. Thông tin backup nằm trong file
`.json` cạnh từng file backup, phiên đăng nhập và chỉ số chỉ giữ trong RAM, người chơi đọc từ file của
Minecraft. SQLite để tới khi có tài khoản và nhật ký thao tác (P5). Spec P1 mục 3.1 ghi "SQLite từ P2",
nay dời lại.

### 3.2 Điều khiển người chơi qua console, không dùng RCON

MineHost đã giữ stdin của server, nên mọi lệnh (`whitelist add`, `kick`, `attribute`…) gửi qua
supervisor như lệnh console. Không cần bật RCON, không phải giữ mật khẩu RCON. Danh sách đang online
lấy từ log: dòng `<tên> joined the game` / `left the game`, cộng một lần `list` ngay khi server sang
`Running` để không lệch.

### 3.3 Đã đặt mật khẩu thì mở từ chính máy này cũng phải đăng nhập

Người dùng chọn cách này. Chưa đặt mật khẩu thì không hỏi gì, giống P1. Quên mật khẩu thì đặt lại
bằng một trong hai cách, đều cần ngồi trước máy:

- Lệnh `minehost reset-password` (Windows và Linux). Lệnh xóa mật khẩu và tắt LAN, chạy được cả khi
  MineHost đang chạy vì controller đọc lại `auth.toml` khi file đổi.
- Trên Windows, menu khay "Đặt lại mật khẩu". Bấm lần đầu thì chữ đổi thành "Bấm lần nữa để xóa mật
  khẩu" trong 10 giây (khay không có hộp thoại, ADR 0002).

Ghi lại thành ADR 0003.

### 3.4 Giao diện qua LAN chỉ có HTTP

Chưa có TLS, nên mật khẩu đi qua mạng LAN dạng không mã hóa. Trang Cài đặt ghi rõ cảnh báo này cạnh
công tắc LAN. TLS xét ở P4 cùng truy cập từ Internet.

### 3.5 Chỉ số chỉ giữ 1 giờ trong RAM

Lấy mẫu mỗi 5 giây, giữ 720 mẫu mỗi server. Tắt MineHost thì mất. Đủ để xem "vừa rồi có gì bất
thường" mà không cần nơi lưu trữ.

### 3.6 Xóa file là chuyển vào thùng rác

Giống xóa server ở P1: file hoặc thư mục bị xóa trong trang Tệp được chuyển sang
`<data>/trash/<server-id>/files/<thời điểm>/<đường dẫn cũ>`. Trang Tệp ghi rõ đường dẫn này.

### 3.7 Backup là tar.zst có file mô tả đi kèm

Theo spec gốc: `backups/<server-id>/<backup-id>.tar.zst` và `<backup-id>.json`. Không cần chỉ mục
nào khác: liệt kê backup là quét thư mục.

### 3.8 Skin lấy qua MineHost

Giao diện không gọi thẳng tới máy chủ của Mojang. MineHost tải skin, lưu cache, rồi trả về cho giao
diện. Nhờ vậy trang vẫn hiện skin đã có khi mất mạng, và trình duyệt không phải nói chuyện với
domain nào ngoài MineHost.

## 4. Mô hình dữ liệu

### 4.1 Thêm vào `instance.toml` (vẫn schema 1)

Các bảng mới đều tùy chọn. Bản P1 đọc file có các bảng này vẫn chạy, vì khóa lạ được giữ nguyên.

```toml
[backup]
schedule = "every_6h"        # "off" | "every_<n>h" (1..168) | "daily@HH:MM"
keep = 10                    # số bản tự động giữ lại, 1..100

[policy]
restart_at = ["04:00"]       # giờ khởi động lại hằng ngày (giờ máy), tối đa 4 mốc
```

### 4.2 File mô tả backup

```json
{
  "id": "01JC…",
  "server_id": "01JB…",
  "kind": "manual",           // "manual" | "scheduled" | "before_restore"
  "created_at": "2026-10-26T10:00:00Z",
  "size": 734003200,
  "sha256": "…",
  "minecraft": "1.21.4",
  "loader": "paper",
  "note": "Trước khi đổi phiên bản",
  "protected": false
}
```

- Ghi bằng `write_atomic` sau khi file `.tar.zst` đã hoàn chỉnh. File `.tar.zst` không có `.json`
  đi kèm là backup dở, bị xóa lúc controller khởi động.
- Giữ lại: chỉ áp dụng cho `scheduled`, bỏ qua bản `protected`. Bản `manual` giữ tới khi người dùng
  xóa. Bản `before_restore` giữ 3 bản mới nhất.

### 4.3 `<data>/auth.toml`

```toml
password_hash = "$argon2id$v=19$m=19456,t=2,p=1$…"
lan = false
```

- Không có file hoặc không có `password_hash` thì chưa đặt mật khẩu.
- Ghi bằng `write_atomic`; trên Linux đặt quyền `600`.
- `lan = true` chỉ hợp lệ khi có `password_hash`; đọc thấy `lan = true` mà thiếu mật khẩu thì coi như tắt.

### 4.4 Người chơi

Đọc từ thư mục server, không lưu bản riêng:

| Nguồn | Dùng cho |
| --- | --- |
| `usercache.json` | Người chơi đã từng vào: tên, UUID, lần cuối vào |
| `whitelist.json`, `ops.json`, `banned-players.json` | Trạng thái whitelist, op, bị cấm (kèm lý do) |
| Log console (mục 3.2) | Đang online |
| `servers/<id>/frozen.json` (MineHost ghi) | Ai đang bị đóng băng, vì Minecraft không có lệnh xem lại |

### 4.5 Mã lỗi mới trong `ErrorCode`

| Mã | Khi nào |
| --- | --- |
| `SERVER_NOT_RUNNING` | Thao tác cần server đang chạy: đuổi, đóng băng, dịch chuyển, nhắn tin |
| `FILE_TOO_LARGE` | Mở file lớn hơn 1 MiB trong trình sửa, hoặc tải lên quá 512 MiB |
| `FILE_NOT_TEXT` | File không phải UTF-8 hợp lệ |
| `FILE_CHANGED` | Lưu file nhưng file trên đĩa đã đổi kể từ lúc mở (`If-Match` không khớp) |
| `FILE_EXISTS` | Tạo hoặc đổi tên trùng file đã có |
| `BACKUP_PROTECTED` | Xóa backup đang được bảo vệ |
| `BACKUP_CORRUPT` | Hash của file backup không khớp với file mô tả |
| `PLAYER_NOT_FOUND` | Không tìm thấy tài khoản Minecraft có tên này (server bật online-mode) |
| `UNSUPPORTED_BY_VERSION` | Tính năng không có ở phiên bản Minecraft này (ví dụ đóng băng dưới 1.16) |
| `AUTH_REQUIRED` | Chưa đăng nhập |
| `AUTH_INVALID` | Sai mật khẩu |
| `AUTH_RATE_LIMITED` | Sai quá nhiều lần, kèm `retry_after_s` |
| `PASSWORD_TOO_SHORT` | Mật khẩu mới dưới 8 ký tự |
| `LAN_NEEDS_PASSWORD` | Bật LAN khi chưa đặt mật khẩu |
| `MODS_NOT_SUPPORTED` | Nhập thư mục server có `mods/` (mod có từ P3) |

## 5. Thiết kế từng crate

Không thêm cạnh phụ thuộc mới giữa các crate: mọi phần ghép nằm trong `mh-app`.

### 5.1 `mh-core`

- `players`: hàm thuần nhận một dòng log và trả `Joined(name)` / `Left(name)` / `List { online, max, names }`;
  `offline_uuid(name)` (UUID v3 của `"OfflinePlayer:" + name`, như Minecraft tính); tạo lệnh cho từng
  thao tác ở mục 6.7, có kiểm tra tên người chơi (`[A-Za-z0-9_]{3,16}`) và escape JSON cho `tellraw`.
- `freeze_attributes(mc_version)`: `movement_speed` (mặc định 0.1), thêm `jump_strength` (mặc định 0.42)
  từ 1.20.5; trước 1.21.2 tên có tiền tố `generic.`. Dưới 1.16 (chưa có lệnh `attribute`) trả
  `UNSUPPORTED_BY_VERSION`. Thứ tự phiên bản lấy từ `releaseTime` (spec P1 mục 3.5), không tự so sánh chuỗi.
- `BackupSchedule` (parse `every_6h`, `daily@04:00`), `fn next_run(schedule, last, now)` thuần, có test.
- Các mã lỗi ở mục 4.5.

### 5.2 `mh-platform`

- `process_usage(pid) -> { cpu_percent, rss_bytes }` dùng `sysinfo` (CPU tính trên toàn máy, 0..100).
- `dir_size(path)`: tổng kích thước, không theo symlink.
- Test fuzz cho `SafeRoot::join` (`cargo-fuzz`, thư mục `fuzz/` ở gốc repo).

### 5.3 `mh-store`

- Đọc ghi `whitelist.json`, `ops.json`, `banned-players.json`, `usercache.json`, `frozen.json` qua
  `write_atomic`. Giữ nguyên trường lạ khi ghi lại.
- Đọc ghi bảng `[backup]` và `policy.restart_at` của `instance.toml`.

### 5.4 `mh-providers`

- Mojang: `profile_by_name(name) -> { uuid, name }`
  (`https://api.mojang.com/users/profiles/minecraft/<name>`), `skin(uuid) -> png`
  (`https://sessionserver.mojang.com/session/minecraft/profile/<uuid>`, rồi tải ảnh trong `textures`).
  Chỉ cho tải ảnh từ `textures.minecraft.net`. Cache trong `cache/skins/<uuid>.png`, sống 24 giờ.
- Vanilla: `downloads.server` của Mojang (đã có từ P1).
- Purpur: `https://api.purpurmc.org/v2/purpur/<version>/latest`, kiểm tra md5 (thêm `Checksum::Md5`
  vào `mh-net`).

### 5.5 `mh-supervisor`

- `send_and_wait(line, pattern, timeout)`: gửi một lệnh rồi chờ dòng log khớp `pattern`. Backup dùng
  hàm này cho `save-all flush` → chờ `Saved the game`.
- Phát thêm sự kiện `Players(Joined | Left | List)` từ log, dùng hàm của `mh-core`.

### 5.6 `mh-backup`

- `create(src_dir, dest, exclude, progress, cancel) -> { size, sha256 }`: ghi `dest.part`, xong thì
  `rename`. Không theo symlink. Loại trừ `logs/`, `crash-reports/`, `cache/`.
- `restore(archive, sha256, dest_dir, progress, cancel)`: kiểm tra hash trước, giải nén qua `SafeRoot`,
  bỏ qua symlink, hardlink và file thiết bị.
- `zstd` mức 3. Test fuzz cho bộ giải nén.

### 5.7 `mh-app`

- **Job mới:** `backup`, `restore`, `import` (mục 6.11). Mọi Job hiện trong trang Tác vụ.
- **Backup khi server đang chạy:** `save-off` → `save-all flush` → chờ `Saved the game` (tối đa 60 giây)
  → nén → `save-on`. `save-on` luôn được gửi, kể cả khi nén lỗi hoặc bị hủy. Server tắt thì nén luôn.
  Trước khi nén kiểm tra ổ đĩa còn trống ít nhất bằng kích thước thư mục server.
- **Khôi phục:** server phải đang tắt; nếu đang chạy thì Job tắt server trước (hộp thoại đã báo trước).
  Tạo backup `before_restore` của hiện trạng → giải nén vào `servers/<id>/.restore-<ulid>/` → đổi tên
  `server/` thành `.old-<ulid>/` → đổi `.restore-<ulid>/` thành `server/` → xóa `.old-<ulid>/`.
  Lỗi trước bước đổi tên thì xóa thư mục tạm, `server/` giữ nguyên. Không tự bật lại server.
  Controller chết giữa hai lần đổi tên: lúc khởi động và khi reconcile, xóa mọi `.restore-*`; nếu
  thiếu `server/` thì đổi `.old-*` mới nhất về `server/`, rồi xóa các `.old-*` còn lại.
- **Lịch backup:** một task kiểm tra mỗi phút. Server không chạy kể từ bản backup trước thì bỏ qua.
  Lần kiểm tra đầu sau khi controller khởi động chờ 5 phút, để không backup ngay lúc mọi server đang bật.
  `every_<n>h` tính từ bản `scheduled` gần nhất; `daily@HH:MM` theo giờ máy, controller tắt đúng giờ đó
  thì lần bật sau sao lưu bù một lần. Chưa có bản nào thì sao lưu ngay. Xong bản `scheduled` thì xóa
  bản `scheduled` không bảo vệ vượt `keep`.
- **Lịch khởi động lại:** tới giờ thì gửi `say` báo trước 5 phút, 1 phút, 10 giây, rồi `restart`.
  Server đang tắt thì bỏ qua. Kiểm tra mỗi giây theo giờ máy; controller bật khi chỉ còn ít phút thì
  chỉ gửi lời báo đang tới hạn.
- **Chỉ số:** mỗi 5 giây lấy CPU, RAM của process Java và số người online; mỗi 5 phút tính lại dung lượng
  thư mục server. Vòng đệm 720 mẫu mỗi server, chỉ khi server có process.
- **Cảnh báo RAM:** tổng `max` RAM của các server đang chạy cộng server sắp bật vượt RAM máy trừ 2 GB thì
  `start` vẫn chạy nhưng trả kèm `warnings: ["memory_overcommit"]`.
- **Tệp:** gốc là `servers/<id>/server/`, mọi đường dẫn qua `SafeRoot::join`. Chặn sửa và xóa
  `server.jar` khi server đang chạy.
- **Mật khẩu:** băm bằng `argon2id` (tham số mặc định của thư viện `argon2`). Đọc lại `auth.toml` khi
  thời điểm sửa file đổi (mục 3.3).

### 5.8 `mh-api`

REST mới (mọi thao tác dài trả `202` kèm `job_id`):

| Method | Path | Ghi chú |
| --- | --- | --- |
| GET | `/jobs?server_id=` | 50 Job gần nhất, mới nhất trước |
| GET | `/servers/{id}/metrics?since=` | Mẫu sau thời điểm `since`, dung lượng đĩa, thời gian chạy |
| GET | `/servers/{id}/files?path=` | Liệt kê thư mục: tên, loại, kích thước, thời điểm sửa |
| GET / PUT | `/servers/{id}/files/content?path=` | Văn bản ≤ 1 MiB kèm `ETag`; PUT bắt buộc `If-Match` |
| GET | `/servers/{id}/files/download?path=` | Tải một file |
| POST | `/servers/{id}/files/upload?path=` | `multipart/form-data`, mỗi file ≤ 512 MiB |
| POST | `/servers/{id}/files/mkdir` · `/files/rename` | `{ path }` · `{ from, to }` |
| DELETE | `/servers/{id}/files?path=` | Chuyển vào thùng rác (mục 3.6) |
| GET | `/servers/{id}/players` | Online, đã biết, whitelist, op, bị cấm, đóng băng; `online_mode`, `max` |
| POST | `/servers/{id}/players/{name}/actions` | `{ action, reason?, target?, text? }` (mục 6.7) |
| GET | `/players/{uuid}/skin` | Ảnh PNG từ cache (mục 3.8) |
| GET / POST | `/servers/{id}/backups` | Danh sách · `{ note?, protected? }` → Job |
| PATCH / DELETE | `/servers/{id}/backups/{bid}` | Sửa ghi chú, bảo vệ · xóa |
| GET | `/servers/{id}/backups/{bid}/download` | Tải file `.tar.zst` |
| POST | `/servers/{id}/backups/{bid}/restore` | `{ "confirm": true }` → Job |
| POST | `/servers/import/inspect` | `{ path }` → `{ loader, minecraft, jar, has_mods }`, xem trước, không chép |
| POST | `/servers/import` | `{ path, name, loader?, minecraft? }` → Job (mục 6.11); `loader`, `minecraft` chỉ gửi khi chọn tay |
| GET | `/auth/session` | `{ password_set, logged_in, csrf_token? }`, không cần đăng nhập |
| POST | `/auth/login` · `/auth/logout` | `{ password }` |
| PUT / DELETE | `/auth/password` | `{ current?, new }` · `{ current }` (xóa mật khẩu thì tắt LAN) |
| GET / PUT | `/settings/access` | `{ lan, lan_urls }` |

- `PATCH /servers/{id}` nhận thêm `backup` và `restart_at`. `POST /servers` nhận thêm `loader` là
  `vanilla` hoặc `purpur`; thêm `GET /catalog/{vanilla|purpur}/versions`.
- `GET /system` thêm `cpu_percent`, `memory_used_bytes`, `disk_free_bytes`.
- WebSocket thêm topic `server.<id>.metrics` (mỗi 5 giây) và `server.<id>.players`.
- Upload là route duy nhất nhận body không phải JSON; vẫn phải qua kiểm tra Origin và CSRF.

### 5.9 Binary `minehost`

- Lệnh `minehost reset-password` (mục 3.3).
- **LAN:** bật thì mở thêm một listener trên từng địa chỉ IPv4 trong LAN, cùng cổng với `127.0.0.1`.
  Quét lại địa chỉ mỗi 60 giây (DHCP đổi IP). Tắt thì đóng các listener đó, không cần khởi động lại.
  Kiểm tra `Host` chấp nhận thêm `<ip-lan>:<cổng>` của chính các listener đang mở.
- Menu khay thêm "Đặt lại mật khẩu" (Windows).

## 6. Các trang

Mỗi trang chỉ ghép từ component trong `ui/src/ui/` (spec `p1-ui-components.md`). Component mới ở mục 6.12.

### 6.1 Trang chủ `/`

- Thẻ số: server đang chạy / tổng, người chơi online, tác vụ đang chạy, RAM máy đã dùng / tổng, ổ đĩa
  còn trống.
- Danh sách ngắn các server đang chạy: tên, trạng thái, người chơi, nút mở.
- Chưa có server: giữ màn hình "Tạo server đầu tiên" của P1.

### 6.2 Máy chủ `/servers`

- Bảng: tên, trạng thái, phiên bản, địa chỉ, người chơi, RAM. Tìm theo tên, lọc theo trạng thái, sắp xếp theo cột.
- Mỗi dòng có nút Bật / Tắt / Khởi động lại và menu "…" (mở console, sao lưu ngay).
- Nút "Tạo máy chủ" và "Nhập server có sẵn".

### 6.3 Tác vụ `/jobs`

Bảng Job: loại, server, trạng thái, tiến độ, thời điểm bắt đầu, lỗi (đã dịch). Job đang chạy có nút Hủy.
Lọc theo server. Cập nhật realtime qua WebSocket.

### 6.4 Tổng quan server `/servers/:id`

- Thẻ số: CPU, RAM (so với `max` đã đặt), người chơi online / tối đa, thời gian chạy, dung lượng thư mục.
- Biểu đồ đường 1 giờ gần nhất cho CPU, RAM và người chơi. Server tắt thì ghi "Server đang tắt", không vẽ.
- Job gần nhất của server và thông báo crash (giữ từ P1).

### 6.5 Console `/servers/:id/console`

Giữ console của P1, thêm: ô lọc dòng, nút Tạm dừng / Tiếp tục cuộn, nút Xóa màn hình (chỉ xóa phía
trình duyệt). Không đổi gì ở backend.

### 6.6 Tệp `/servers/:id/files`

- Đường dẫn dạng breadcrumb, bảng tên / kích thước / sửa lúc, thư mục trước, file sau.
- Nút: tệp mới, thư mục mới, tải lên (nút hoặc kéo thả, có thanh tiến độ), tải xuống, đổi tên, xóa.
- Bấm vào file văn bản ≤ 1 MiB thì mở trình sửa `/files/edit`: ô văn bản font mono, nút Lưu
  (Ctrl+S), cảnh báo khi rời trang còn thay đổi chưa lưu. Gặp `FILE_CHANGED` thì cho chọn "Tải lại
  bản mới" hoặc "Chép thay đổi của tôi".
- File lớn hơn hoặc không phải văn bản: chỉ tải xuống.

### 6.7 Người chơi `/servers/:id/players`

- Hai bảng: Đang online, Đã từng vào. Mỗi người có đầu skin, tên, huy hiệu op / whitelist / bị cấm /
  đóng băng. Ô thêm người chơi theo tên (để whitelist hoặc cấm người chưa vào bao giờ).
- Bấm vào một người thì mở hộp thoại chi tiết: skin 3D xoay được, UUID có nút chép, các nhóm nút:

| Nhóm | Thao tác | Lệnh | Khi server tắt |
| --- | --- | --- | --- |
| Quyền | Thêm / gỡ whitelist, cấp / thu hồi op | `whitelist add/remove`, `op`, `deop` | Sửa file JSON |
| Kỷ luật | Đuổi, cấm, bỏ cấm (kèm lý do) | `kick`, `ban`, `pardon` | Cấm, bỏ cấm: sửa file; đuổi: không được |
| Tương tác | Đóng băng / bỏ đóng băng | `attribute <p> <attr> base set 0` / đặt lại giá trị mặc định (mục 5.1) | Không được |
| | Dịch chuyển tới người chơi khác, tới điểm spawn | `tp <p> <đích>`, `execute in minecraft:overworld run tp <p> <spawn>` | Không được |
| | Nhắn tin, thông báo giữa màn hình | `tellraw`, `title <p> title` | Không được |

Khi server tắt, MineHost ghi UUID giống cách Minecraft sẽ tính: offline-mode tính từ tên; online-mode
lấy từ `usercache.json` nếu người đó đã vào (bỏ UUID offline còn sót), không có thì hỏi Mojang. Mất
mạng thì báo `NETWORK_UNAVAILABLE`, không ghi UUID đoán. Server đang bật hoặc đang tắt dở thì mọi thao
tác trả `SERVER_BUSY`. Topic `server.<id>.players` gửi cả danh sách mỗi lần có người vào, ra, hoặc
file người chơi đổi.

- Sửa file khi server tắt cần UUID: `online-mode=true` thì hỏi Mojang (`profile_by_name`),
  `false` thì dùng `offline_uuid`. Điểm spawn đọc từ `level.dat`.
- Thao tác không làm được khi server tắt thì nút bị mờ, kèm chú thích "Cần bật server".
- Skin 3D dùng `skinview3d`, chỉ tải khi mở hộp thoại. Không có skin (offline-mode, mất mạng) thì
  dùng skin mặc định của Minecraft.

### 6.8 Sao lưu `/servers/:id/backups`

- Thẻ "Sao lưu ngay": ghi chú, công tắc "Bảo vệ bản này", nút Sao lưu.
- Thẻ "Tự động": lịch (Tắt / Mỗi N giờ / Hằng ngày lúc), số bản giữ lại. Ghi rõ "Bản tạo tay và bản được
  bảo vệ không bị tự xóa".
- Bảng: thời điểm, loại, kích thước, phiên bản Minecraft, ghi chú, khóa bảo vệ. Menu mỗi dòng: Khôi phục,
  Tải xuống, Sửa ghi chú, Bảo vệ / Bỏ bảo vệ, Xóa.
- Khôi phục: hộp thoại ghi "World và cấu hình hiện tại sẽ được thay bằng bản lúc <thời điểm>. MineHost
  sẽ sao lưu hiện trạng trước." Server đang chạy thì thêm "Server sẽ được tắt". Phải tích ô "Tôi hiểu"
  mới bấm được.

### 6.9 Cài đặt game và Cài đặt server

Giữ hai tab của P1, chuyển thành hai mục trên thanh bên. Cài đặt server thêm: giờ khởi động lại hằng ngày.
Cài đặt game thêm mô tả ngắn cho từng khóa phổ biến của `server.properties`, như dự án cũ.

### 6.10 Cài đặt `/settings` và Đăng nhập `/login`

- **Bảo mật:** đặt, đổi, xóa mật khẩu (đổi và xóa phải nhập mật khẩu hiện tại).
- **Truy cập từ LAN:** công tắc, chỉ bật được khi đã có mật khẩu; khi bật thì hiện các địa chỉ
  `http://192.168.x.x:7878` có nút chép, kèm cảnh báo của mục 3.4.
- **Thông tin:** phiên bản MineHost, thư mục dữ liệu, RAM và ổ đĩa của máy.
- **Đăng nhập:** một ô mật khẩu và nút Đăng nhập. Sai quá 5 lần thì khóa 1 phút, mỗi lần sau gấp đôi,
  tối đa 15 phút; trang hiện thời gian còn phải chờ. Đăng nhập xong quay về trang đang mở dở.

### 6.11 Nhập server có sẵn

Trong wizard tạo server, bước 1 thêm lựa chọn "Nhập thư mục có sẵn": người dùng dán đường dẫn thư mục
trên máy. MineHost nhận loại server từ jar (Paper, Purpur, Vanilla) và phiên bản từ `version_history.json`
hoặc `version.json` trong jar; không nhận ra thì cho chọn tay. Thư mục được **chép** vào
`servers/<id>/server/`, bản gốc giữ nguyên; symlink bị bỏ qua. Server có mod (`mods/`) thì báo
`MODS_NOT_SUPPORTED`. Giữ cổng trong `server.properties` nếu còn trống. EULA vẫn phải bấm đồng ý lại
trong MineHost dù thư mục đã có `eula=true`.

### 6.12 Component mới

| Component | Dùng ở |
| --- | --- |
| `AppShell` thêm `variant="sidebar"`, `NavItem`, `NavSection` | mọi trang |
| `StatTile` (nhãn, số, đơn vị, dòng phụ) | Trang chủ, Tổng quan |
| `LineChart` (SVG tự vẽ, một hoặc nhiều đường, trục thời gian, tooltip) | Tổng quan |
| `Breadcrumb` | Tệp |
| `FileDrop` (kéo thả, nút chọn file, danh sách tiến độ) | Tệp |
| `TextArea` thêm `variant="code"` | Sửa tệp |
| `Avatar` (đầu skin, chữ cái thay thế) | Người chơi |

## 7. Phụ thuộc mới

| Crate | Thư viện | Lý do |
| --- | --- | --- |
| `mh-backup` | `tar`, `zstd` | Nén và giải nén backup |
| `mh-app` | `argon2`, `rand` | Băm mật khẩu, token phiên và CSRF |
| `mh-app` | `jiff` | Giờ máy theo múi giờ cho lịch khởi động lại; `time` không đọc được múi giờ an toàn khi process có nhiều luồng |
| `mh-api` | `axum` feature `multipart`, `axum-extra` (cookie) | Tải file lên, cookie phiên |
| `mh-providers` | `fastnbt` | Đọc điểm spawn trong `level.dat` |
| `mh-core` | `md-5` | UUID offline của người chơi |
| `mh-net` | `md-5` | Kiểm tra file Purpur |
| `ui` | `skinview3d` | Skin 3D, tải lười |
| `ui` (dev) | `@playwright/test` | Test E2E mỗi trang |
| `fuzz/` | `libfuzzer-sys` | Fuzz `SafeRoot` và giải nén |

## 8. Bảo mật

- Đã đặt mật khẩu: mọi route trừ `/auth/session`, `/auth/login` và file tĩnh của UI đều cần cookie
  `mh_session` (`HttpOnly`, `SameSite=Strict`, `Path=/`). Phiên sống 30 ngày hoặc tới khi MineHost tắt.
- Request thay đổi dữ liệu có phiên phải gửi `X-CSRF-Token` khớp với phiên. Vẫn giữ kiểm tra Origin của P1.
- Giới hạn đăng nhập sai theo địa chỉ IP (mục 6.10). So sánh token bằng hàm thời gian hằng.
- Mọi đường dẫn file qua `SafeRoot::join`; symlink trỏ ra ngoài thư mục server bị từ chối khi đọc, ghi và xóa.
- Giải nén backup: kiểm tra hash trước, chặn zip-slip, bỏ symlink và hardlink.
- Lệnh người chơi chỉ nhận tên hợp lệ; nội dung tin nhắn được escape JSON, không ghép chuỗi thô vào lệnh.
- Skin chỉ tải từ `sessionserver.mojang.com` và `textures.minecraft.net`.

## 9. Test

| Lớp | Nội dung chính |
| --- | --- |
| Unit | Phân tích log người chơi, `offline_uuid`, thuộc tính đóng băng theo phiên bản, lịch backup `next_run`, escape lệnh |
| Store | Đọc ghi các file JSON của Minecraft giữ trường lạ; bảng `[backup]` |
| Backup | Nén rồi giải nén ra đúng nội dung; archive có `../`, symlink, hardlink bị chặn; sai hash thì `BACKUP_CORRUPT`; hủy giữa chừng không để lại `.part` |
| App (`fake-mc`) | Backup khi đang chạy gửi đúng thứ tự `save-off`, `save-all`, `save-on`, kể cả khi lỗi; khôi phục lỗi giữa chừng không mất `server/`; lịch giữ lại đúng số bản |
| API | Mỗi route một test `oneshot`; chưa đăng nhập bị `AUTH_REQUIRED`; sai CSRF bị 403; khóa sau 5 lần sai; đường dẫn `../` bị `PATH_OUTSIDE_ROOT` |
| Tích hợp (CI) | Paper thật: backup, phá world, khôi phục, khối còn đó (tiêu chí mục 1); whitelist khi server tắt rồi bật vẫn còn |
| Fuzz (hằng tuần) | `SafeRoot::join`, giải nén backup |
| E2E (Playwright) | Mỗi trang ở mục 6 một luồng chính, chạy với binary thật và `fake-mc` |
| Chạy dài (tay) | 10 server, 72 giờ, ghi RSS của controller mỗi 10 phút |

## 10. Rủi ro

| Rủi ro | Cách giảm |
| --- | --- |
| Backup world lớn làm server giật | `save-off` chỉ trong lúc nén; nén ở mức 3; ưu tiên thấp cho luồng nén |
| Khôi phục hỏng giữa chừng mất world | Giải nén ra thư mục tạm, đổi tên ở bước cuối, luôn có bản `before_restore` |
| Định dạng log người chơi khác nhau giữa các bản Minecraft | Fixture log từ 1.16, 1.20, 1.21, 26.x; gửi `list` để đồng bộ lại |
| Quên mật khẩu khi đã bắt đăng nhập cả trên máy này | `reset-password` và menu khay (mục 3.3); hướng dẫn trong README |
| Mật khẩu lộ qua LAN vì chưa có TLS | Cảnh báo trong UI; LAN tắt mặc định; TLS ở P4 |
| `skinview3d` kéo theo three.js làm UI nặng | Tách chunk, chỉ tải khi mở hộp thoại người chơi |

## 11. Quyết định khi duyệt (2026-10-26)

1. Vanilla, Purpur, lịch khởi động lại và nhập server có sẵn **giữ trong P2**, làm sau các trang giao
   diện (kế hoạch PR nhóm H).
2. Xóa file trong trang Tệp **chuyển vào thùng rác** (mục 3.6).
3. Khôi phục khi server đang chạy thì **MineHost tự tắt server** rồi khôi phục (mục 5.7).
4. P2 xong thì phát hành **`v0.3.0`**.
5. Đồng ý các phần thêm: test Playwright với binary thật, chặn sửa và xóa `server.jar` khi server đang
   chạy, tải lên tối đa 512 MiB mỗi file, trình sửa file chỉ mở văn bản ≤ 1 MiB và không tô màu cú pháp.

## 12. Vấn đề mở sau lần rà soát (2026-11-11)

Chưa có quyết định. Chi tiết và bằng chứng nằm ở
[rà soát kiến trúc 2026-11-11](../ra-soat/2026-11-11-kien-truc.md); mục này chỉ ghi những điểm chạm
vào spec P2 để không bị quên trước khi phát hành `v0.3.0`.

| Mục spec | Vấn đề | Cần quyết định |
| --- | --- | --- |
| 5.7 Khôi phục | Chỉ kiểm tra dung lượng trống trước khi tạo bản `before_restore`, không kiểm tra trước khi giải nén vào `.restore-<ulid>/`. Lúc đó trên đĩa có cùng lúc world cũ, bản backup mới và world giải nén, nên ổ có thể đầy giữa chừng | Kiểm tra dung lượng trống so với kích thước giải nén ước tính trước bước giải nén, thiếu thì trả `NOT_ENOUGH_DISK`. Thêm vào PR-22 (#47) hoặc một PR ngay sau |
| 4.2 File mô tả backup | `instance.toml` không nằm trong `.tar.zst`, và `.json` không ghi RAM, cờ JVM, cổng. Khôi phục vào chính server đó thì không sao, nhưng người dùng tải backup về cất rồi cài lại máy sẽ mất cấu hình. Cũng ảnh hưởng PR-32 (nhập server) nếu sau này nhập từ file backup | Chép bản chụp `instance.toml` vào archive (khôi phục bỏ qua nó, chỉ dùng khi nhập), hay ghi các trường cấu hình vào `.json` |
| 3.4 LAN chỉ có HTTP | Mật khẩu admin cho phép gửi lệnh console và tải plugin lên, tức là quyền chạy code trên máy host. Mật khẩu đó đi qua LAN dạng không mã hóa. Crafty Controller mặc định dùng HTTPS tự ký | Giữ như đã duyệt (chỉ cảnh báo), hay đưa TLS tự ký vào cùng PR-28 |
| 5.7 Cảnh báo RAM | `memory_overcommit` cộng `max` (`-Xmx`) của các server. JVM còn dùng bộ nhớ ngoài heap nên con số thật cao hơn | Tính mỗi server là `max × 1,25 + 256 MB` (con số ước lượng, cần đo lại trong lần chạy 72 giờ), hoặc dùng RSS đo được với server đang chạy |
| 1 Tiêu chí 72 giờ | Đo trên Linux, nhưng bản Linux chưa có cách chạy bền (đóng SSH là controller dừng) | Dùng unit systemd ở [ops/linux-systemd.md](../ops/linux-systemd.md) cho lần chạy dài |
