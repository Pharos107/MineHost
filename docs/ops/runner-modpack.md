# Runner tự host cho workflow modpack

- Trạng thái: cài trên máy thử nghiệm Linux (dangserver, Ubuntu 26.04, 12 nhân, 31 GB RAM) ngày
  2026-11-15, runner `v2.338.2`.
- Quyết định gốc: [spec P3 mục 3.11](../specs/p3-mod-va-modpack.md#311-kiểm-thử-modpack-trên-runner-tự-host).
  Workflow dùng runner này: `.github/workflows/modpacks.yml` (PR-37).

## Vì sao cần

Repo private nên runner của GitHub chỉ có khoảng 7 GB RAM và bị giới hạn phút chạy. Một modpack lớn
cần tới 10 GB RAM và vài phút mỗi lần bật. Workflow `modpacks` vì vậy chạy trên một runner đặt ở
máy thử nghiệm, lúc 04:00 giờ Việt Nam, lần lượt từng pack.

Máy này còn chạy dịch vụ thật (một server Minecraft có người chơi và vài dịch vụ khác), nên runner
bị nhốt trong một unit systemd có giới hạn chặt.

## Runner được làm gì

| Giới hạn | Giá trị | Lý do |
| --- | --- | --- |
| User | `mh-runner`, user hệ thống, không có `sudo`, không thuộc nhóm `docker` | Code trong workflow không chiếm được máy |
| Thư mục | Mọi thứ trong `/srv/mh-runner` (runner, Rust, cache Java và modpack) | `ProtectSystem=strict`: ngoài thư mục này chỉ đọc được |
| `/home` | Không thấy (`ProtectHome=yes`) | Dữ liệu thật trên máy nằm trong `/home` |
| RAM | `MemoryMax=12G`, `MemorySwapMax=0` | Vượt thì kernel kill test, không làm server thật bị hết RAM hay bị đẩy sang swap |
| CPU, đĩa | `CPUWeight=20`, `IOWeight=20`, `Nice=10` | Khi tranh tài nguyên, runner luôn nhường |
| Mạng | Chỉ Internet và localhost; chặn LAN, Docker, Tailscale (`IPAddressDeny`) | Test không với được thiết bị khác trong nhà |
| Loopback | Kết nối mới của user `mh-runner` chỉ tới DNS (53) và cổng test 25610, 25611 (bảng nftables `inet mh_runner`, nạp khi unit bật, gỡ khi tắt) | Dịch vụ chỉ nghe ở `127.0.0.1` trên máy (MineHost thật ở 7878, password manager, RCON) không bị code trong workflow gọi tới |
| Process | `KillMode=control-group`, `TasksMax=8192` | Dừng runner là dừng hết Java của test |
| cgroup | `Delegate=yes` | MineHost tạo được cgroup con để giới hạn RAM từng server khi chạy được |

Unit đầy đủ nằm trong [`scripts/modpack-runner.sh`](../../scripts/modpack-runner.sh) (hàm `write_unit`).
Muốn đổi giới hạn thì sửa ở đó, cài lại rồi sửa bảng trên.

Chỉ workflow có `runs-on: [self-hosted, minehost-modpack]` mới chạy trên runner này. Repo private
nên không có pull request từ fork chạy được ở đây.

## Cài đặt

Cần quyền admin repo trên GitHub (để lấy token) và `sudo` trên máy.

1. Lấy registration token (sống 1 giờ) và ghi vào một file chỉ mình đọc được trên máy:

   ```bash
   gh api -X POST repos/Pharos107/MineHost/actions/runners/registration-token --jq .token | ssh dangserver 'umask 077; cat > /tmp/mh-runner-token'
   ```

2. Trên máy, chạy script với file đó. Script tải runner, kiểm tra SHA-256, tạo user, đăng ký runner
   với nhãn `minehost-modpack`, ghi unit rồi bật nó. File token bị xóa ngay sau khi đọc.

   ```bash
   sudo bash scripts/modpack-runner.sh install /tmp/mh-runner-token
   ```

3. Kiểm tra: `sudo bash scripts/modpack-runner.sh status` phải báo `active (running)`; trên GitHub,
   Settings → Actions → Runners hiện `dangserver-modpack` ở trạng thái Idle.

Runner tự cập nhật bản mới khi GitHub yêu cầu. Khi đổi `RUNNER_VERSION` trong script thì đổi luôn
`RUNNER_SHA256` (lấy ở trang release của `actions/runner`).

## Gỡ

```bash
gh api -X POST repos/Pharos107/MineHost/actions/runners/remove-token --jq .token | ssh dangserver 'umask 077; cat > /tmp/mh-runner-token'
```

```bash
sudo bash scripts/modpack-runner.sh uninstall /tmp/mh-runner-token
```

Script dừng và xóa unit, hủy đăng ký runner trên GitHub, xóa user `mh-runner` và cả `/srv/mh-runner`
(gồm cache Java và modpack). Không xóa gì ngoài các thứ đó.

## Khi có sự cố

- Xem log: `journalctl -u mh-runner -n 200`.
- Server thật bị chậm lúc 04:00: hạ `CPUWeight` hoặc dời giờ chạy trong `modpacks.yml`.
- Test bị kill vì RAM (`memory.events` có `oom_kill`): pack đó cần quá 12 GB, giảm RAM của pack
  trong `tests/modpacks.toml` hoặc bỏ pack khỏi danh sách.
- Hết chỗ đĩa: xóa `/srv/mh-runner/modpack-data/cache` (lần chạy sau tải lại từ đầu).
