# P4 — Kết quả khảo sát (PR-01)

Trả lời các câu hỏi mà [spec P4](p4-truy-cap-internet.md) để lại cho PR-01: tích hợp playit (mục 3.4),
dịch vụ kiểm tra từ ngoài (mục 3.5), hiệu năng Docker Desktop trên Windows (mục 3.12). Khảo sát ngày
2026-11-19. Code thử nằm ngoài repo, không merge.

## 1. playit.gg

Nguồn đã đọc:

- Mã nguồn agent `playit-cloud/playit-agent` tag `v2.0.2` (phát hành 2026-11-10), các gói
  `playitd`, `playit-cli`, `api_client`.
- Plugin Minecraft chính thức `playit-cloud/playit-minecraft-plugin` (nhánh `master`, đẩy lần cuối
  2026-02-20), các file `PlayitKeysSetup.java`, `PlayitManager.java`.
- [Điều khoản](https://playit.gg/terms) cập nhật 2026-02-20, [bảng giá](https://playit.gg/pricing).

Chưa làm: chưa chạy binary agent, chưa liên kết tài khoản thật. Hai việc này cần tài khoản playit của
chủ dự án, nên dời sang lần thử tay ở PR-06 và PR-10 (mục 1.6).

### 1.1 Liên kết tài khoản: làm được không cần terminal

Cả CLI lẫn plugin chính thức đều liên kết qua HTTP tới `https://api.playit.gg`, không cần TUI:

1. Tạo mã claim ngẫu nhiên dạng hex. Plugin dùng 8 byte, CLI dùng 5 byte.
2. Cho người dùng mở `https://playit.gg/claim/<mã>`.
3. Gọi `POST /claim/setup` với `{code, agent_type, version}` mỗi vài giây. Phản hồi là
   `WaitingForUserVisit`, `WaitingForUser`, `UserAccepted` hoặc `UserRejected`. Nếu lỗi
   `CodeExpired` hay `InvalidCode` thì tạo mã mới.
4. Khi `UserAccepted`: gọi `POST /claim/exchange` với `{code}` để nhận `secret_key`.
5. Các lời gọi sau gửi header `Authorization: Agent-Key <secret>`.

Plugin dùng `agent_type = "self-managed"`. Lệnh `playit claim exchange` cũng mặc định giá trị này.
Người chưa có tài khoản vẫn liên kết được: playit tạo tài khoản khách (`AccountStatus::Guest`), sau
đó đăng nhập qua `https://playit.gg/login/guest-account/<session>`.

Kết luận: `mh-app` tự làm luồng này bằng `mh-net`. Sau khi có secret thì ghi vào `secrets.toml`. Agent
không cần chạy trong lúc liên kết.

### 1.2 Tạo tunnel tự động: làm được về kỹ thuật, điều khoản chưa rõ

Agent 2.0.2 không tự tạo tunnel. Lệnh `tunnels prepare` của CLI đã bị comment từ bản 0.16. Tuy vậy,
plugin Minecraft chính thức của playit tự tạo tunnel bằng agent key:

- `POST /v1/tunnels/list`: liệt kê tunnel. Mỗi tunnel có `tunnel_type`, `origin` (agent và
  `local_port`), `connect_addresses` (địa chỉ cho người chơi).
- `POST /v1/tunnels/create` với:
  - `ports = {type:"tunnel-type", details:"minecraft-java"}`;
  - `origin = {type:"agent", data:{agent_id, config:{fields:[{name:"local_port", value:"25565"}, …]}}}`;
  - `enabled = true`.

  Kết quả trả id tunnel. Plugin đợi tối đa 10 giây để địa chỉ hiện ra trong `list`.
- `POST /tunnels/delete` với `{tunnel_id}`: xóa tunnel.
- `POST /v1/agents/rundata`: id agent, tunnel của agent, thông báo, quyền (`is_self_managed`,
  `has_premium`, `account_status`).

Các kiểu này có trong `api_client` của agent (sinh tự động) và trong client Java của plugin. Đây không
phải tài liệu công khai. Vì playit tự dùng chúng trong sản phẩm của mình, khả năng bị bỏ đột ngột
thấp, nhưng có thể đổi mà không báo trước.

**Điều khoản.** Mục 3.4 của điều khoản cấm bán lại, cấp phép lại, phân phối, hoặc đưa dịch vụ, tunnel
hay băng thông cho bên thứ ba dùng khi chưa có "prior written consent" của Developed Methods LLC. Mỗi
người dùng MineHost tự liên kết tài khoản của chính họ và tunnel trỏ về máy của họ, nên cách dùng của
MineHost giống plugin chính thức hơn là bán lại. Dù vậy, câu chữ không phân biệt "phần mềm bên thứ ba
giúp người dùng tạo tunnel của chính họ". Mục 3.3 cấm dịch vụ "remotely control, manage … multiple
devices". MineHost chỉ điều khiển agent trên chính máy host, nên không thuộc trường hợp đó.

Kết luận: chủ dự án hỏi playit (trang hỗ trợ hoặc Discord) trước khi phát hành tính năng tạo tunnel tự
động. Bản nháp câu hỏi ở mục 1.7. Trong lúc chờ, PR-06 và PR-07 vẫn làm được. Nếu playit không đồng ý
thì chỉ tắt bước tạo tự động, các phần khác giữ nguyên (mục 1.5).

### 1.3 Nhiều server, UDP, giới hạn bản miễn phí

- **Một agent, nhiều tunnel:** được. Tunnel gắn với agent và `local_port`, nên một agent phục vụ mọi
  server trên máy, mỗi server một tunnel.
- **Loại tunnel miễn phí** (theo bảng giá ngày khảo sát):
  - Miễn phí: `minecraft-java`, `minecraft-bedrock` và UDP tùy chỉnh.
  - Cần Premium: TCP tùy chỉnh, TCP+UDP, HTTPS.
  - Lỗi trả về khi tạo có `RequiresPlayitPremium`, `PublicPortRequiresPlayitPremium`,
    `RegionRequiresPlayitPremium`, `RequiresVerifiedAccount`.
- **Số tunnel tối đa** của bản miễn phí: playit không công bố. Các bài viết bên ngoài nói khác nhau
  (1, 3, hay 4 TCP + 4 UDP). Cần đo khi thử tay. MineHost hiện đúng mã lỗi playit trả về, không tự đặt
  giới hạn.
- **Hệ quả cho cổng phụ (spec mục 3.8):**
  - Geyser (UDP 19132) và Simple Voice Chat (UDP 24454) mở qua playit miễn phí được.
  - BlueMap (TCP 8100) và cổng TCP khác cần Premium. Với UPnP thì không bị giới hạn này.

### 1.4 Độ trễ từ Việt Nam

Chưa đo, vì cần tài khoản thật. Bảng giá liệt kê 19 trung tâm dữ liệu. Gần Việt Nam nhất là Singapore
và Nhật; không có máy chủ ở Việt Nam. Bản miễn phí dùng mạng `Global` (anycast); chọn vùng riêng là
Premium (`RegionRequiresPlayitPremium`). Đo ở PR-10: ping từ một máy ở mạng khác qua tunnel và qua
UPnP, ghi vào kết quả thử tay.

### 1.5 Chạy agent như process con

`playitd` chạy được ở foreground, không cần terminal hay quyền admin:

- `--secret-path <file>` đọc file TOML có dòng `secret_key = "…"`. Ngoài ra có `--secret <key>`, nhưng
  cách này để lộ key trong danh sách tiến trình, nên không dùng.
- `--socket-path <đường dẫn>` đổi named pipe hoặc socket IPC. Cần đặt riêng để không đụng bản playit mà
  người dùng có thể đã cài làm service.
- Không có `--log-path` thì log đi ra stderr.
- Ctrl+C (Windows) và SIGINT/SIGTERM (Unix) dừng gọn. Job Object hoặc `PR_SET_PDEATHSIG` của
  `mh-runtime` lo trường hợp controller chết.
- Bản phát hành có binary riêng cho từng nền tảng:
  - Windows: `playitd-windows-x86_64.exe` (7,8 MB) và bản `-signed`.
  - Linux: binary gộp `playit-linux-amd64` và `-aarch64`.

  PR-06 kiểm tra binary Linux có chạy được daemon ở foreground không, hay phải build `playitd`.
- Giấy phép BSD-2-Clause. Tải binary chính thức từ GitHub Releases thì không phải tự build.

Thiết kế chốt cho PR-06 và PR-07:

- Tunnel do MineHost quản lý có tên `MineHost <tên server>`.
- Khi bật server có cách mở playit:
  1. Tìm trong `v1/tunnels/list` tunnel loại `minecraft-java` trỏ về agent này với `local_port` bằng
     cổng server.
  2. Có thì dùng.
  3. Không có thì tạo, chỉ khi cờ tạo tự động đang bật (mục 1.2).
  4. Địa chỉ lấy từ `connect_addresses`. Người dùng không phải dán gì.
- Khi tắt cờ tạo tự động (playit không đồng ý), UI có nút mở trang tạo tunnel của playit kèm hướng dẫn
  điền cổng. MineHost tự nhận tunnel đó ở lần dò sau. Không còn bước dán địa chỉ như mc-controller.
- Đổi cổng server: tunnel cũ do MineHost tạo thì xóa rồi tạo lại.
- Xóa server: chỉ xóa tunnel do MineHost tạo.
- Gỡ liên kết: xóa `playit_secret`. Tunnel giữ nguyên trên tài khoản, UI ghi rõ điều này.

### 1.6 Còn phải thử tay (PR-06, PR-10)

- Liên kết bằng tài khoản thật và bằng tài khoản khách.
- Tạo tunnel `minecraft-java` qua API. Người chơi vào được; đo ping từ Việt Nam.
- Số tunnel tối đa của bản miễn phí; tunnel UDP cho Geyser.
- `playit-linux-amd64` chạy daemon foreground được không.

### 1.7 Câu hỏi gửi playit (chủ dự án gửi)

> MineHost is a desktop app that hosts Minecraft servers on the user's
> own PC. We would like to offer playit.gg as an option: the user clicks "Link playit account", visits
> the claim URL, and MineHost runs the official `playitd` binary (downloaded from your GitHub
> releases) and creates a `minecraft-java` tunnel for each of their servers through
> `/v1/tunnels/create`, the same way your Minecraft plugin does. Each user uses their own account;
> we do not resell or share tunnels. Is this allowed under section 3.4 of your terms, and is there
> anything you would like us to do (agent version string, rate limits, attribution)?

Ghi kết quả trả lời vào đây khi có.

## 2. Dịch vụ kiểm tra từ ngoài

Hai ứng viên đã thử ngày 2026-11-19, gọi với `User-Agent: MineHost/0.5 (survey)`:

```bash
curl -s -A "$UA" -D h.txt "https://api.mcsrvstat.us/3/mc.hypixel.net"
curl -s -A "$UA" -D h.txt "https://api.mcstatus.io/v2/status/java/mc.hypixel.net"
curl -s -A "$UA" "https://api.mcsrvstat.us/3/203.0.113.7:25566"            # không vào được
curl -s -A "$UA" "https://api.mcstatus.io/v2/status/java/203.0.113.7:25566"
curl -s -A "" -o /dev/null -w "%{http_code}" "https://api.mcsrvstat.us/3/mc.hypixel.net"  # 403
```

| Tiêu chí (spec mục 3.5) | `mcsrvstat.us` | `mcstatus.io` |
| --- | --- | --- |
| Không cần key | Đúng, nhưng thiếu `User-Agent` thì trả 403 | Đúng |
| Thử bắt tay Minecraft thật | Có: Server List Ping, Query thì tùy chọn | Có: trả phiên bản, MOTD, số người lấy từ server |
| Thời gian cache | **5 phút** (`cacheexpire - cachetime = 300`), kể cả kết quả "không vào được" | **60 giây** (`Cache-Control: max-age=60`, `expires_at`) |
| Giới hạn tần suất | Không công bố | 5 request/giây mỗi IP; muốn hơn thì gửi email |
| Điều khoản cho phần mềm | Không có; xin quyên góp | Không có; có email hỗ trợ |
| Trường hợp không vào được | `online:false`, lý do trong `debug.errors` | `online:false`, đợi khoảng 5,7 giây mới trả |

Chọn **`mcstatus.io`**:

- Cache 60 giây hợp với luồng "mở cổng xong bấm kiểm tra lại". Với cache 5 phút, người dùng sửa
  router xong vẫn thấy kết quả cũ.
- Giới hạn tần suất có công bố.

`mcsrvstat.us` làm dự phòng, chưa code. Spec mục 3.5 và 5.3 sửa theo: `cached_until` lấy từ
`expires_at`; UI ghi "kết quả có thể cũ tới 60 giây". Fixture cho PR-04 ghi lại từ hai lệnh ở trên.

## 3. Docker Desktop trên Windows

Đo ngày 2026-11-19 trên máy phát triển:

- Máy: Windows 11 Home, Ryzen 7 7840HS, 32 GB RAM, SSD NVMe.
- Docker Desktop 29.8.2 (WSL2, 16 CPU, 16 GB).
- Paper 26.3 (`server.jar` có sẵn trong `.minehost-dev`); Java 25 (Temurin 25.0.4.1, do MineHost tải).
- Image `eclipse-temurin:25-jre@sha256:945247f9d092703e0c6be92243ef3c51640229e23ab34d353c8dcdc73e6cd4e6`.

Lúc đo, một server Paper khác của `.minehost-dev` vẫn chạy rỗi trên máy, nên số có nhiễu. Các lượt chạy
xen kẽ giữa ba cách để nhiễu chia đều.

Ba cách được so:

| Cách | Thư mục server | Lệnh |
| --- | --- | --- |
| Thường | Thư mục trên NTFS | `java -Xms2G -Xmx2G -jar paper.jar --nogui` |
| Docker bind mount | Cùng thư mục NTFS mount vào `/data`: cách spec mục 3.12 định dùng | `docker run --rm -i --network none -v <thư mục>:/data -w /data <image> java …` |
| Docker named volume | Volume nằm trong ổ ext4 của máy ảo WSL2 | như trên với `-v mhbench:/data` |

Mỗi lượt chạy như sau:

1. Thư mục mới, seed `20261119`, `cache/` và `libraries/` chép sẵn để không tính thời gian tải.
2. Chờ dòng `Done (`.
3. Chạy `forceload add` cho 4 vùng 16×16 chunk (1.024 chunk; Paper 26.3 cho tối đa 256 chunk mỗi
   lệnh) và chờ 120 giây.
4. `save-all flush`, rồi `stop`.

Số "lưu chunk khi tắt" lấy từ dòng `[ChunkHolderManager] Saved … block chunks … in Xs` của Paper. Mỗi
lượt lưu khoảng 2.100 chunk, world khoảng 25 MB. Script nằm ngoài repo; mỗi cách chạy hai lượt.

| Cách | Khởi động tới `Done` (giây, Paper tự báo) | Lưu chunk khi tắt (giây) | `stop` tới khi thoát (giây) |
| --- | --- | --- | --- |
| Thường | 11,1 · 8,3 | 0,33 · 0,25 | 1,2 · 1,2 |
| Docker bind mount | 17,0 · 20,9 | 1,20 · 1,50 | 3,3 · 3,8 |
| Docker named volume | 9,1 · 12,6 | 0,18 · 0,18 | 1,8 · 1,6 |

Số `save-all flush` (khoảng 1–2 giây ở cả ba cách) không dùng được. Trong lúc lưu, chunk vẫn đang được
sinh tiếp, nên thời gian phụ thuộc thời điểm hơn là ổ đĩa.

Kết luận:

- **Bind mount từ NTFS so với cách thường:**
  - Khởi động chậm khoảng **2 lần**.
  - Lưu chunk chậm khoảng **4,5 lần**. Vượt ngưỡng 3 lần của spec mục 3.12, nên UI Windows hiện cảnh
    báo cạnh lựa chọn Docker.
  - Thời gian tuyệt đối vẫn nhỏ: khoảng 1,3 giây cho 2.100 chunk. Với server vài người chơi, autosave
    mỗi 5 phút khó gây giật; với server đông người hay nhiều mod thì có thể giật.
- **Named volume** nhanh ngang hoặc hơn cách thường. Nhưng world khi đó nằm trong máy ảo của Docker,
  nên không có trong thư mục server: backup, trình quản lý tệp và `instance.toml` không còn là nguồn dữ
  liệu chính (AGENTS.md). Không dùng.
- **Giữ thiết kế bind mount** của spec. Câu cảnh báo: "Trên Windows, Docker ghi world chậm hơn cách
  thường khoảng 4 lần. Chỉ chọn Docker khi bạn cần cô lập server." Trên Linux, bind mount không đi qua
  lớp chia sẻ file nên không cảnh báo; đo lại ở PR-23 trên dangserver.
