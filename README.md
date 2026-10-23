# MineHost

Host server Minecraft cho bạn bè ngay trên máy của bạn, không cần biết dòng lệnh hay Docker.
Chạy trên Windows 10/11 và Linux (x64, arm64).

> Đang ở giai đoạn P0 (nền móng): chạy `minehost` sẽ mở giao diện web hiển thị thông tin máy.
> Chưa tạo được server Minecraft.

## Chạy thử từ mã nguồn

Cần Rust (bản trong `rust-toolchain.toml` sẽ tự cài) và Node.js 24.

```bash
cd ui && npm ci && npm run build && cd ..
cargo run -p minehost
```

Trình duyệt tự mở `http://127.0.0.1:7878`. Dữ liệu nằm ở `%LOCALAPPDATA%\minehost` (Windows)
hoặc `~/.local/share/minehost` (Linux); xem bằng `cargo run -p minehost -- paths`.

Quy tắc đóng góp và kiến trúc: xem [AGENTS.md](AGENTS.md).
