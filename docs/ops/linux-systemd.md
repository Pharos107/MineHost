# Chạy MineHost bền trên Linux bằng systemd

- Trạng thái: **chưa thử trên máy thật.** Unit dưới đây viết theo hành vi đọc được trong code
  (`crates/minehost/src/serve.rs`, `cli.rs`). Thử xong thì sửa dòng này và ghi lại phiên bản đã thử.
- Service chính thức (cài bằng một lệnh, cả Windows) là việc của P4. Đây là cách tạm.

## Vì sao cần

Chạy `./minehost` trong phiên SSH thì đóng phiên là controller dừng, và server Minecraft là process con
nên dừng theo ([ADR 0001](../adr/0001-server-dung-theo-controller.md)). systemd giữ controller chạy
sau khi bạn thoát SSH, tự bật lại khi máy khởi động lại hoặc controller crash.

## Cài đặt

Tạo user riêng và thư mục. MineHost không cần quyền root và không nên chạy bằng root.

```bash
sudo useradd --system --create-home --home-dir /var/lib/minehost --shell /usr/sbin/nologin minehost
```

```bash
sudo install -o root -g root -m 0755 minehost /usr/local/bin/minehost
```

Tạo file `/etc/systemd/system/minehost.service`:

```ini
[Unit]
Description=MineHost
After=network-online.target
Wants=network-online.target

[Service]
User=minehost
Group=minehost
ExecStart=/usr/local/bin/minehost --data-dir /var/lib/minehost/data --no-browser
Restart=on-failure
RestartSec=5

# SIGTERM chỉ gửi cho controller để nó tự gửi `stop` tới từng server. Mặc định systemd gửi cho
# cả Java, khiến server tự tắt cùng lúc controller đang ra lệnh.
KillMode=mixed
KillSignal=SIGTERM
# Mỗi server có tối đa 60 giây để lưu world; chừa thêm cho controller.
TimeoutStopSec=120
# Giao cgroup cho MineHost để giới hạn RAM của từng server (spec P3, mục 3.8). Thiếu dòng này thì
# server chỉ bị giới hạn bằng -Xmx.
Delegate=yes

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
```

```bash
sudo systemctl enable --now minehost
```

## Mở giao diện

MineHost chỉ lắng nghe `127.0.0.1:7878`. Từ máy của bạn, mở một đường hầm SSH rồi vào
`http://127.0.0.1:7878`:

```bash
ssh -L 7878:127.0.0.1:7878 <user>@<địa-chỉ-vps>
```

Giữ đúng cổng 7878 ở cả hai đầu: MineHost từ chối request có header `Host` khác `127.0.0.1:7878` hoặc
`localhost:7878`. Đừng đổi `listen` trong `config.toml` thành `0.0.0.0`: bản hiện tại chưa có mật
khẩu, và kiểm tra `Host` sẽ chặn truy cập bằng địa chỉ khác.

Cổng game (mặc định 25565) thì cần mở trên tường lửa của VPS để người chơi vào.

## Dùng hằng ngày

| Việc | Lệnh |
| --- | --- |
| Xem trạng thái | `systemctl status minehost` |
| Xem log controller | `journalctl -u minehost -f` (log file nằm ở `/var/lib/minehost/data/logs/`) |
| Tắt an toàn mọi server rồi dừng | `sudo systemctl stop minehost` |
| Cập nhật | Dừng service, thay `/usr/local/bin/minehost`, bật lại |

Server nào tự bật lại khi service khởi động:

- Sau khi **tắt an toàn** (`systemctl stop`, cập nhật): chỉ server có `autostart = true`. Bật tùy chọn
  này trong Cài đặt server cho những server cần chạy liên tục.
- Sau khi controller **bị kill hoặc máy mất điện**: thêm cả server đang chạy lúc đó (còn dấu `.running`).

## Tùy chọn: giới hạn và sandbox

Chưa thử. Các dòng này thêm vào mục `[Service]`, áp dụng cho cả controller lẫn mọi server Java vì
chúng cùng một cgroup. Đây là cách sớm nhất để có giới hạn cứng trên Linux khi MineHost chưa tự cô lập
process ([bao-mat.md](../architecture/bao-mat.md#lỗ-hổng-đã-biết), mục A1).

```ini
# Tổng RAM của controller và mọi server; vượt thì kernel kill một process trong nhóm (thường là
# Java lớn nhất).
MemoryMax=12G
# Mặc định (`stop`) systemd dừng cả service khi kernel kill một process vì hết RAM, kéo theo mọi
# server. `continue` để supervisor coi đó là crash của riêng server ấy.
OOMPolicy=continue
# Không cho leo quyền, chỉ cho ghi vào thư mục dữ liệu, không thấy thư mục home của user khác.
NoNewPrivileges=yes
ProtectSystem=strict
ReadWritePaths=/var/lib/minehost
ProtectHome=yes
PrivateTmp=yes
```

Không bật `MemoryDenyWriteExecute`: JVM cần vùng nhớ vừa ghi vừa chạy cho JIT.
