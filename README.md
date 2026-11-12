# MineHost

Host server Minecraft cho bạn bè ngay trên máy của bạn, không cần biết dòng lệnh, không cần cài
Java hay Docker. Chạy trên Windows 10/11 và Linux (x64, arm64).

Bản hiện tại (v0.2.0) tạo và chạy được server **Paper**: MineHost tự tải Java và Paper, có console
realtime, sửa `server.properties`, và chạy nền với icon ở khay hệ thống trên Windows.

## Tải và chạy

1. Vào trang [Releases](https://github.com/Pharos107/MineHost/releases) và tải bản cho máy của bạn:
   - Windows: `minehost-v0.2.0-x86_64-pc-windows-msvc.zip`
   - Linux x64: `minehost-v0.2.0-x86_64-unknown-linux-gnu.tar.gz`
   - Linux arm64 (ví dụ Raspberry Pi 5): `minehost-v0.2.0-aarch64-unknown-linux-gnu.tar.gz`
2. Giải nén vào một thư mục bất kỳ.
3. Windows: bấm đúp `minehost.exe`. Linux: chạy `./minehost` trong terminal.
4. Trình duyệt tự mở `http://127.0.0.1:7878`. Bấm **Tạo server đầu tiên** và làm theo 3 bước.

Lần đầu tạo server, MineHost tải Java (~60 MB) và Paper (~50 MB); mạng 20 Mbps mất khoảng một phút.

### Windows báo "Windows protected your PC" (SmartScreen)

Bản này chưa được ký số nên Windows SmartScreen có thể chặn lần chạy đầu. Bấm **More info**
(Thông tin thêm) rồi **Run anyway** (Vẫn chạy). Chỉ làm vậy với file tải từ trang Releases ở trên;
có thể so mã SHA-256 với file `.sha256` đi kèm:

```powershell
Get-FileHash .\minehost-v0.2.0-x86_64-pc-windows-msvc.zip -Algorithm SHA256
```

Một số phần mềm diệt virus cảnh báo vì MineHost chạy Java ở chế độ ẩn. Đó là server Minecraft
của bạn; MineHost ghi log mỗi lần chạy vào thư mục `logs/` (xem mục Dữ liệu).

## Chơi cùng bạn bè trong mạng LAN

1. Trên trang server, bấm **Bật server**. Lần đầu, đọc và bấm **Tôi đồng ý** với EULA của Minecraft.
2. Chờ trạng thái **Đang chạy**.
3. Chép địa chỉ hiện dưới tên server (ví dụ `192.168.1.5:25565`) gửi cho bạn bè dùng chung Wi-Fi
   hoặc mạng nhà.
4. Trong Minecraft: **Multiplayer → Add Server**, dán địa chỉ, rồi vào chơi. Bạn bè phải dùng đúng
   phiên bản Minecraft của server.

Bạn bè không vào được:

- Windows có thể hỏi "Allow access" cho Java lần đầu bật server: cho phép với **Private networks**.
  Nếu lỡ chặn, vào Windows Defender Firewall → Allow an app → tìm `java.exe` → bật Private.
- Kiểm tra hai máy cùng một mạng (khách Wi-Fi thường bị tách riêng).
- Chơi qua Internet (không cùng mạng) chưa có ở bản này; dự kiến ở P4.

## Hằng ngày

- **Đóng tab trình duyệt** không tắt gì cả; server vẫn chạy. Mở lại bằng chuột phải icon MineHost
  ở khay → **Mở MineHost** (Windows), hoặc chạy `minehost` lần nữa: bản thứ hai không chạy song
  song mà chỉ mở lại trình duyệt.
- **Thoát hẳn**: bấm **Thoát** trên thanh trên của giao diện, hoặc chuột phải icon ở khay → Thoát.
  MineHost gửi lệnh `stop` cho từng server để lưu world rồi mới tắt.
- **Xóa server** chuyển thư mục server vào `trash/`; muốn lấy lại thì chép ngược vào `servers/`.

## Dữ liệu

| Hệ điều hành | Thư mục |
| --- | --- |
| Windows | `%LOCALAPPDATA%\minehost` |
| Linux | `~/.local/share/minehost` |

Bên trong: `servers/` (mỗi server một thư mục, world nằm trong `server/world`), `java/`, `logs/`,
`cache/`, `backups/`, `trash/`, `config.toml`. Dùng thư mục khác bằng `minehost --data-dir <thư mục>`.
Từng file và thư mục dùng để làm gì: [docs/architecture/du-lieu.md](docs/architecture/du-lieu.md).

Chạy trên VPS Linux: `./minehost` dừng khi bạn đóng phiên SSH, kéo theo mọi server. Muốn chạy bền
thì dùng unit systemd trong [docs/ops/linux-systemd.md](docs/ops/linux-systemd.md).

## Chạy từ mã nguồn

Cần Rust (bản trong `rust-toolchain.toml` sẽ tự cài) và Node.js 24.

```bash
cd ui && npm ci && npm run build && cd ..
cargo run -p minehost -- --data-dir .minehost-dev
```

Quy tắc đóng góp và lộ trình: [AGENTS.md](AGENTS.md). Tài liệu kỹ thuật (kiến trúc, bảo mật, dữ
liệu, vận hành): [docs/README.md](docs/README.md).
