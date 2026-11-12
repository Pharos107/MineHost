# Thư mục dữ liệu

- Khớp code tới: 2026-11-11 (`origin/main` ở `6a03ff8`)
- Vị trí mặc định: Windows `%LOCALAPPDATA%\minehost`, Linux `~/.local/share/minehost`. Đổi bằng
  `--data-dir` hoặc biến `MINEHOST_DATA_DIR`.

Nguyên tắc: thư mục trên đĩa là nguồn dữ liệu chính. Không có cơ sở dữ liệu; mọi thứ không nằm trong
bảng dưới đây (Job, chỉ số, bộ đệm console) chỉ ở trong RAM và mất khi controller tắt.

## Cấu trúc

```text
<data>/
├── config.toml                 cấu hình controller
├── minehost.lock               khóa "chỉ một bản chạy"
├── runtime.json                { url, pid } của bản đang chạy
├── logs/                       log của controller
├── java/<major>/               Temurin JRE, dùng chung giữa các server
├── cache/
│   ├── meta/                   bản tốt gần nhất của JSON từ Mojang, Paper, Adoptium
│   └── <sha1|sha256>/<2 ký tự>/<hash>   file đã tải, dùng lại theo hash
├── servers/<server-id>/
│   ├── instance.toml           nguồn chính của server này
│   ├── .running                dấu "đang chạy", để bật lại sau khi controller chết
│   ├── frozen.json             người chơi đang bị đóng băng (P2)
│   ├── logs/console-YYYY-MM-DD.log   console do controller ghi, giữ 14 ngày
│   └── server/                 thư mục làm việc của Minecraft: world, plugins, server.properties
├── backups/<server-id>/
│   ├── <backup-id>.tar.zst     nội dung thư mục server/
│   └── <backup-id>.json        file mô tả; thiếu file này thì bản backup bị coi là dở
└── trash/
    ├── <server-id>-<thời điểm>/            server đã "xóa"
    └── <server-id>/files/<thời điểm>/...   file đã "xóa" trong trang Tệp
```

## Từng file

| File | Ai ghi | Định dạng, tham chiếu | Mất hoặc hỏng thì sao |
| --- | --- | --- | --- |
| `config.toml` | Controller tạo lần đầu; người dùng sửa tay | `listen`, `open_browser`, `log_level` | Thiếu file: tạo lại với mặc định (`127.0.0.1:7878`). Hỏng hoặc có khóa lạ: controller báo `CONFIG_INVALID` và không khởi động |
| `minehost.lock`, `runtime.json` | Controller | Bản thứ hai đọc `runtime.json` để mở trình duyệt rồi thoát | Tự tạo lại |
| `java/<major>/` | `mh-java` | Giải nén vào thư mục tạm rồi đổi tên | Tải lại khi cần |
| `cache/` | `mh-net` | Theo hash | Xóa được; chỉ phải tải lại |
| `instance.toml` | `mh-store`, ghi nguyên tử | Schema 1: [spec P1 mục 4.1](../specs/p1-mot-server-chay-duoc.md#41-instancetoml-schema-1), thêm `[backup]` và `policy.restart_at` ở [spec P2 mục 4.1](../specs/p2-nhieu-server-va-van-hanh.md#41-thêm-vào-instancetoml-vẫn-schema-1). Khóa lạ được giữ nguyên | Server hiện là `Broken`. `schema` lớn hơn bản hiện tại cũng là `Broken` |
| `.running` | Supervisor | File rỗng | Server không tự bật lại sau khi controller chết (trừ khi `autostart`) |
| `server/eula.txt` | Controller, **chỉ sau khi** người dùng bấm đồng ý | `eula=true` | Ghi lại lúc bật nếu `instance.toml` có `eula.accepted_at` |
| `server/server.properties` | Controller và Minecraft | Giữ thứ tự dòng và comment. `server-port` bị ghi đè theo `instance.toml` trước mỗi lần bật | Minecraft tạo lại với mặc định |
| `server/whitelist.json`, `ops.json`, `banned-players.json`, `usercache.json` | Minecraft; MineHost sửa khi server tắt | Giữ trường lạ | Mất danh sách tương ứng |
| `<backup-id>.json` | Job sao lưu, ghi sau khi `.tar.zst` hoàn chỉnh | [Spec P2 mục 4.2](../specs/p2-nhieu-server-va-van-hanh.md#42-file-mô-tả-backup) | `.tar.zst` không có `.json` bị xóa lúc controller khởi động |

File sẽ có khi PR tương ứng merge: `<data>/auth.toml` (mật khẩu admin, PR-26).

## Quy tắc ghi

- File cấu hình ghi bằng `mh_platform::fs::write_atomic` (ghi file tạm, rồi đổi tên).
- File tải về và backup ghi vào `<tên>.part`, kiểm tra hash, rồi đổi tên.
- Đường dẫn nhận từ người dùng đi qua `SafeRoot::join`; gốc của trang Tệp là `servers/<id>/server/`.
- Xóa server và xóa file đều là chuyển vào `trash/`. MineHost chưa bao giờ tự dọn `trash/`.

## Sao lưu chứa gì

Toàn bộ `servers/<id>/server/`, trừ `logs/`, `crash-reports/`, `cache/` ở gốc và mọi symlink. Mỗi bản
là một file nén đầy đủ (zstd mức 3), không có bản tăng dần. `instance.toml` **không** nằm trong backup.

Giải nén tay khi không có MineHost (cần `tar` có hỗ trợ zstd):

```bash
tar --zstd -xf <backup-id>.tar.zst -C <thư mục đích>
```

## Chuyển server sang máy khác

Chép cả thư mục `servers/<server-id>/` sang `servers/` của máy mới khi cả hai controller đang tắt.
Controller quét `servers/*/instance.toml` lúc khởi động nên server sẽ hiện ra; Java thiếu thì được tải
lại ở lần bật đầu. Hai điều cần giữ:

- Tên thư mục phải đúng bằng `id` trong `instance.toml`, nếu không server hiện là `Broken`.
- Xóa file `.running` trong bản chép nếu không muốn server tự bật ở máy mới.

Cách này dựa trên thiết kế, chưa có test tự động; luồng "Nhập server có sẵn" chính thức là PR-32 của P2.
