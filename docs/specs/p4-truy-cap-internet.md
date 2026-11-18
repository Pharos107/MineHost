# Spec P4 — Chơi qua Internet, Docker, chạy cùng máy, tự cập nhật

- Trạng thái: đã duyệt (#174). Mục 3.4, 3.5, 5.2, 5.3 sửa theo [khảo sát](p4-khao-sat.md) ở PR-01.
- Ngày: 2026-11-18
- Spec gốc: "MineHost v2 — Spec kiến trúc (Rust)", phần Runtime, Bảo mật, Đóng gói và Lộ trình P4
- Kế hoạch PR: [p4-ke-hoach-pr.md](p4-ke-hoach-pr.md)
- Tham khảo: dự án cũ mc-controller (`Server MC Host/internal/playit`, `internal/network`, tài liệu
  `docs/superpowers/specs/2026-07-19-network-access-providers-design.md`)

## 1. Mục tiêu

Người host gửi cho bạn bè một địa chỉ, bạn bè ở nhà khác vào chơi được, người host không phải mở
trang quản lý router và không phải hiểu NAT là gì. Cụ thể:

- Mỗi server có công tắc "Chơi qua Internet" với hai cách: tự mở cổng router (UPnP) hoặc tunnel
  playit.gg. MineHost chọn sẵn cách hợp với mạng của người dùng và nói rõ vì sao.
- Kiểm tra từ bên ngoài xem server có vào được không, và nói lý do khi không được (router không có
  UPnP, nhà mạng dùng CGNAT, tường lửa).
- Không cho mở ra Internet một server mà ai cũng vào được bằng tên của op (`online-mode=false` mà
  không bật whitelist).
- Giao diện quản lý qua LAN dùng HTTPS (đóng lỗ hổng A3).
- MineHost tự chạy khi đăng nhập Windows; trên Linux cài service bằng một lệnh.
- Cập nhật MineHost bằng một cú bấm, có chữ ký; bản mới không chạy được thì tự quay về bản cũ.
- Tùy chọn chạy server trong Docker (Windows và Linux) để mod, plugin không đọc được file của người
  dùng; chuyển qua lại với cách chạy thường mà world không đổi.

### Tiêu chí hoàn thành

- [ ] Một người ở mạng khác (không chung LAN, không VPN) vào được server Paper trên máy Windows của
      người host qua địa chỉ playit, người host không đụng tới router. Ghi ở
      [kết quả thử tay P4](p4-ket-qua-thu-tay.md) (tạo ở PR-10). Kèm test tự động với agent playit giả.
- [ ] Trên máy có router hỗ trợ UPnP và IP công khai thật: bật "Tự mở cổng router", bấm "Kiểm tra từ
      Internet" thì báo vào được; tắt server thì ánh xạ cổng bị gỡ. Thử tay, kèm test với IGD giả.
- [ ] Server `online-mode=false` chưa bật whitelist không mở ra Internet được (cả hai cách), có nút
      bật whitelist (test API và Playwright).
- [ ] Bật LAN thì giao diện qua địa chỉ LAN chỉ phục vụ HTTPS; gõ `http://` thì được chuyển sang
      `https://`; cookie phiên có `Secure` (test tích hợp của binary).
- [ ] Cùng một server Paper chuyển Process → Docker → Process, world và người chơi giữ nguyên: test
      tích hợp trên Linux trong CI; trên Windows có Docker Desktop thì thử tay.
- [ ] Controller crash khi server đang chạy trong Docker: lần bật sau gắn lại đúng container, console
      và lệnh vẫn dùng được, không có container mồ côi (test tích hợp Linux).
- [ ] Cập nhật từ bản A lên bản B qua manifest có chữ ký; bản B giả hỏng (thoát ngay lúc khởi động)
      thì sau tối đa 3 lần MineHost tự quay về bản A và báo lỗi (test tích hợp với hai binary dựng
      sẵn, trên Windows và Linux). Manifest sai chữ ký bị từ chối.
- [ ] Windows: bật "Khởi động cùng Windows" thì đăng nhập lại MineHost tự chạy, có icon khay.
      Linux: `minehost service install` tạo unit systemd user chạy được sau khi đăng xuất SSH.
- [ ] Mỗi trang và mục mới ở mục 6 có ít nhất một test Playwright chạy với API thật (nguồn ngoài giả).

### Không làm trong P4

App desktop Tauri (khay và trình duyệt đã đủ, xem mục 3.9); Windows service thật chạy trước khi đăng
nhập; gói `.deb`, MSI; Cloudflare Tunnel, Tailscale, ngrok làm cách mở ra Internet (mục 3.3); mở giao
diện quản lý ra Internet; tự cài bản cập nhật mà không hỏi; cập nhật không làm gián đoạn người chơi;
macOS; Geyser và Floodgate một cú bấm (chỉ cấp cổng phụ cho chúng, mục 3.8); proxy Velocity; tên miền
riêng; chặn mod đọc file khi chạy không Docker (Landlock, AppContainer vẫn ở backlog).

## 2. Bố cục và điều hướng

Giữ bố cục trang server đã chốt ([làm mới trang server](lam-moi-trang-server.md)): không thêm băng
ở đầu trang.

| Ngữ cảnh | Thêm | Đường dẫn |
| --- | --- | --- |
| Tổng quan của server | Thẻ "Địa chỉ" gộp địa chỉ LAN đang có với địa chỉ Internet, kèm nút chép và trạng thái | `/servers/:id` |
| Cài đặt server | Mục "Chơi qua Internet": cách mở, kiểm tra từ ngoài, cổng phụ | `/servers/:id/settings#internet` |
| | Mục "Cách chạy": Thường / Docker | `/servers/:id/settings#runtime` |
| Tạo máy chủ | Chế độ Nâng cao thêm ô "Cách chạy" (chỉ hiện khi có Docker) | `/new` |
| Cài đặt chung | playit.gg: liên kết tài khoản, trạng thái agent, gỡ | `/settings#playit` |
| | Khởi động cùng máy | `/settings#startup` |
| | Cập nhật MineHost | `/settings#updates` |
| | Docker: trạng thái, phiên bản | `/settings#docker` |
| | LAN (đang có): thêm dấu vân tay chứng chỉ HTTPS | `/settings#lan` |
| Thanh trên | Chấm báo khi có bản MineHost mới | mọi trang |

## 3. Quyết định riêng của P4

### 3.1 Chỉ mở cổng game, không mở giao diện quản lý

Mật khẩu admin tương đương quyền chạy code trên máy host ([bao-mat.md](../architecture/bao-mat.md#quyền-admin-nghĩa-là-gì)).
P4 chỉ đưa cổng game (và cổng phụ, mục 3.8) ra Internet. Quản lý từ xa thì dùng Tailscale hoặc
mạng riêng tương tự; README có hướng dẫn ngắn (đã chạy được nhờ #116). Không có route, cờ hay
tunnel nào trỏ tới cổng 7878.

### 3.2 Hai cách mở ra Internet, chọn theo server

| | Tự mở cổng router (UPnP) | Tunnel playit.gg |
| --- | --- | --- |
| Cần gì | Router bật UPnP, IP công khai thật (không CGNAT) | Tài khoản playit.gg miễn phí, liên kết một lần |
| Địa chỉ cho bạn bè | `IP công khai:cổng` (đổi khi nhà mạng cấp IP mới) | Tên miền playit cấp, không đổi |
| Độ trễ | Trực tiếp | Qua máy chủ của playit, phụ thuộc vùng (cần đo, mục 3.4) |
| IP nhà người host | Lộ cho người chơi | Bị che |
| Chạy ở đâu | Trong controller (thư viện `igd-next`) | Agent chính thức của playit, process con của controller |

`instance.toml` ghi cách đã chọn (mục 4.1). Công tắc chỉ có tác dụng khi server đang chạy: bật server
thì mở, tắt (kể cả crash) thì đóng. Controller tắt thì mọi thứ đóng theo, giống server
([ADR 0001](../adr/0001-server-dung-theo-controller.md)).

Gợi ý cách mở: lần đầu người dùng bật công tắc, MineHost thử UPnP trước (mất vài giây). Router trả
IP ngoài là địa chỉ riêng (`10/8`, `172.16/12`, `192.168/16`, `100.64/10`) hoặc khác IP công khai thấy
từ ngoài thì đó là CGNAT: báo "Nhà mạng của bạn dùng chung IP (CGNAT), mở cổng router không giúp được"
và gợi ý playit. Không có router UPnP cũng gợi ý playit.

### 3.3 Chỉ playit.gg làm tunnel trong P4

Backlog có đề nghị thêm Cloudflare Tunnel, Tailscale, ngrok. Không nhận vì cả ba không hợp với người
chơi Minecraft Java phổ thông:

- Cloudflare Tunnel cho TCP thô (Minecraft) đòi máy người chơi cũng chạy `cloudflared`.
- Tailscale đòi mỗi người chơi cài Tailscale và được mời vào mạng; Funnel chỉ cho HTTPS.
- ngrok bản miễn phí đổi địa chỉ TCP mỗi lần chạy và đòi xác minh thẻ cho tunnel TCP.

Ghi lại ở mục "Đã cân nhắc và không làm" của backlog. Tailscale vẫn là cách được hướng dẫn để
**quản lý** từ xa (mục 3.1).

### 3.4 Tích hợp playit

Agent playit là mã nguồn mở (BSD-2-Clause, `playit-cloud/playit-agent`). Bản 2.x tách thành daemon
`playitd` và CLI. Kết quả đầy đủ ở [khảo sát P4](p4-khao-sat.md#1-playitgg) (PR-01, agent 2.0.2).
Tóm tắt những gì đã chốt:

- **Liên kết tài khoản:** `mh-app` tự gọi `/claim/setup` và `/claim/exchange` trên `api.playit.gg` qua
  `mh-net`, với `agent_type = self-managed`, giống plugin Minecraft chính thức của playit. Người dùng
  bấm "Liên kết tài khoản" rồi bấm đồng ý trên trang `playit.gg/claim/<mã>`, không cần terminal.
  Chưa có tài khoản thì playit tạo tài khoản khách.
- **Tunnel cho từng server:** khi server có cách mở playit được bật, MineHost tìm trong
  `/v1/tunnels/list` tunnel `minecraft-java` trỏ về agent này với `local_port` bằng cổng server.
  - Không có thì tạo bằng `/v1/tunnels/create` và đặt tên `MineHost <tên server>`.
  - Địa chỉ cho người chơi lấy từ `connect_addresses`. Không có bước dán địa chỉ như mc-controller.
  - Đổi cổng server, hay xóa server: chỉ xóa và tạo lại tunnel do MineHost tạo.
- **Điều khoản:** API này không có tài liệu công khai. Mục 3.4 của điều khoản playit đòi đồng ý bằng
  văn bản khi "distribute" dịch vụ cho bên thứ ba. Chủ dự án hỏi playit trước khi phát hành
  (khảo sát mục 1.7).
  - Bước tạo tự động nằm sau một cờ trong code, chỉ bật khi playit đồng ý.
  - Khi cờ tắt: UI mở trang tạo tunnel của playit kèm hướng dẫn điền cổng, rồi MineHost tự nhận tunnel
    ở lần dò sau.
- **Giới hạn bản miễn phí:** `minecraft-java` và UDP miễn phí. TCP tùy chỉnh cần Premium, nên cổng phụ
  TCP như BlueMap không mở được qua playit miễn phí (mục 3.8). Số tunnel tối đa không công bố;
  MineHost hiện đúng lỗi playit trả về.
- **Độ trễ:** chưa đo, vì cần tài khoản thật. Bản miễn phí đi qua mạng anycast toàn cầu; máy chủ gần
  nhất ở Singapore và Nhật. Đo khi thử tay ở PR-10.

Cách chạy agent:

- Tải binary chính thức từ GitHub Releases của playit, phiên bản và SHA-256 ghim trong code MineHost
  (như Java: chỉ dùng agent do MineHost tải). Đổi phiên bản agent là một PR.
- Lệnh: `playitd --secret-path <data>/playit/secret.toml --socket-path <đường dẫn riêng>`.
  - Socket riêng để không đụng bản playit người dùng có thể đã cài làm service.
  - Log đi ra stderr.
  - File secret do MineHost viết từ `secrets.toml` lúc bật agent, quyền chỉ người dùng đọc được, xóa
    khi agent tắt.
  - Không dùng `--secret`, vì key sẽ lộ trong danh sách tiến trình.
- Chạy qua `mh-runtime` như process con có Job Object hoặc `PR_SET_PDEATHSIG`; supervisor riêng cho
  agent: chết thì bật lại sau 5, 15, 30 giây, quá 3 lần trong 10 phút thì báo lỗi và dừng thử.
- Chỉ chạy khi có ít nhất một server đang bật với cách mở playit.
- Secret của agent lưu trong `<data>/secrets.toml` cạnh key CurseForge (spec gốc muốn dùng keyring
  của OS; giữ một chỗ cho mọi bí mật, xem mục 8). Không bao giờ trả ra API, log hay backup.

### 3.5 Kiểm tra từ bên ngoài

Kiểm tra cổng từ chính máy host không chứng minh được gì (router thường không cho đi vòng). MineHost
không có máy chủ riêng, nên dùng một dịch vụ công khai thử kết nối Minecraft từ ngoài: API
`api.mcstatus.io/v2/status/java/<địa chỉ>` (khảo sát mục 2). Dịch vụ này không cần key, thử bắt tay
Minecraft thật, cache 60 giây và có công bố giới hạn 5 request/giây. `mcsrvstat.us` cache 5 phút nên
chỉ để dự phòng.

- Chỉ chạy khi người dùng bấm "Kiểm tra từ Internet". Nút ghi rõ: "MineHost sẽ gửi địa chỉ công khai
  và cổng của server tới <dịch vụ> để thử kết nối từ ngoài."
- Với tunnel playit, kiểm tra địa chỉ playit; với UPnP, kiểm tra `IP công khai:cổng`.
- Kết quả: vào được (kèm phiên bản và MOTD đọc được), không vào được (kèm lý do đoán từ bước trước:
  router từ chối ánh xạ, CGNAT, tường lửa Windows chặn Java), hoặc dịch vụ lỗi.
- IP công khai chỉ lấy từ UPnP (`GetExternalIPAddress`); không gọi thêm dịch vụ "IP của tôi" nào.
  `mcstatus.io` không trả IP người gọi, nên CGNAT chỉ nhận ra khi IP router báo là địa chỉ riêng (mục
  3.2). Trường hợp hiếm CGNAT cấp IP công khai thì hiện là "không vào được" kèm các lý do có thể.
- Kết quả ghi giờ lấy (`expires_at` của dịch vụ) và câu "kết quả có thể cũ tới 60 giây".

Tường lửa Windows: lần đầu Java mở cổng, Windows hỏi người dùng. MineHost không tự thêm luật (cần
quyền Administrator, mục 3.6); trang Cài đặt mạng hướng dẫn bấm "Cho phép" và cách kiểm tra lại.

### 3.6 Chặn mở ra Internet khi ai cũng vào được

Trước khi mở (lúc bật server có công tắc bật, và lúc lưu `server.properties` khi server đang mở):

| `online-mode` | `white-list` | Kết quả |
| --- | --- | --- |
| `true` | bất kỳ | Cho mở |
| `false` | `true` | Cho mở, hiện cảnh báo: người lạ biết tên trong whitelist vẫn vào được bằng tên đó; dẫn tới mục "Server offline-mode an toàn" của README (viết ở PR-09, nhận từ backlog) |
| `false` | `false` | Chặn với `INTERNET_REQUIRES_WHITELIST`; nút "Bật whitelist" |

Khi bị chặn lúc bật server: server vẫn chạy trong LAN, chỉ phần Internet không mở, thẻ Địa chỉ ghi lý
do. Đang mở mà người dùng tắt whitelist thì đóng ngay phần Internet. MineHost không ghi thêm thuộc
tính nào khác vào `server.properties`.

### 3.7 HTTPS cho giao diện qua LAN

Từ P4, listener LAN (cả địa chỉ Tailscale) chỉ phục vụ HTTPS. Listener `127.0.0.1` vẫn là HTTP vì
không rời khỏi máy.

- Chứng chỉ tự ký tạo lần đầu bật LAN bằng `rcgen`, ECDSA P-256, hạn 10 năm, SAN gồm tên máy và các
  địa chỉ LAN lúc tạo. Lưu `<data>/tls/cert.pem` và `key.pem` (Linux quyền `600`). Không tạo lại khi
  IP đổi: trình duyệt đã cho phép một lần thì không hỏi lại; nút "Tạo chứng chỉ mới" trong Cài đặt.
- Cùng một cổng nhận cả hai: byte đầu là `0x16` (TLS ClientHello) thì bắt tay TLS, còn lại thì trả
  `308` sang `https://` cùng host. Nhờ vậy địa chỉ cũ người dùng đã lưu vẫn dùng được.
- Cookie phiên đi qua LAN có thêm `Secure`. WebSocket qua `wss://`.
- Mục LAN trong Cài đặt hiện dấu vân tay SHA-256 của chứng chỉ, kèm hướng dẫn: trình duyệt cảnh báo
  lần đầu, so dấu vân tay rồi bấm tiếp tục. Trang chỉ hiện dấu vân tay khi mở từ `127.0.0.1` để người
  dùng có một nguồn đáng tin để so.
- Không dùng Let's Encrypt (cần tên miền công khai) và không cài CA vào máy (cần quyền Administrator,
  dễ bị lạm dụng).

### 3.8 Cổng phụ

Plugin như Geyser (UDP 19132), Simple Voice Chat (UDP 24454), BlueMap (TCP 8100) mở cổng riêng.
`instance.toml` có danh sách cổng phụ (mục 4.1), người dùng thêm tay trong Cài đặt server:

- Kiểm tra trùng với cổng game và cổng phụ của server khác, như kiểm tra cổng của P2.
- Cổng phụ có cờ `internet`: bật thì được ánh xạ UPnP hoặc tunnel playit cùng lúc với cổng game; khi
  chạy trong Docker thì được publish. playit miễn phí chỉ cho UDP tùy chỉnh. Cổng phụ TCP với cách mở
  playit thì UI ghi "cần playit Premium" và hiện lỗi `RequiresPlayitPremium` nếu playit từ chối.
- MineHost không đọc file cấu hình của plugin để đoán cổng và không sửa chúng; mỗi dòng có ô ghi chú.

### 3.9 Không làm Tauri

Spec gốc muốn app desktop Tauri. Chủ dự án quyết định bỏ ngày 2026-11-18: controller đã chạy nền có
icon khay (Windows) và giao diện chạy trong trình duyệt, nên tiêu chí "đóng app thì server vẫn chạy"
đã đúng. Tauri thêm một binary, phụ thuộc WebView2, đóng gói riêng mà không thêm tính năng. Ghi vào
backlog.

### 3.10 Khởi động cùng máy

Chủ dự án chọn ngày 2026-11-18: Windows dùng tự chạy khi đăng nhập, không làm Windows service thật
(service chạy ở session 0, không có icon khay, chạy bằng tài khoản khác nên thư mục dữ liệu phải đổi
chỗ).

- **Windows:** công tắc "Khởi động cùng Windows" ghi giá trị `MineHost` vào
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, trỏ tới đúng đường dẫn exe đang chạy kèm
  `--no-browser`. Exe nằm trong `Downloads` hoặc thư mục tạm thì cảnh báo nên chuyển đi trước. Không
  cần quyền Administrator. Dùng crate `winreg`, không thêm `unsafe`.
- **Linux:** lệnh `minehost service install` ghi unit systemd **user** vào
  `~/.config/systemd/user/minehost.service` theo mẫu ở [ops/linux-systemd.md](../ops/linux-systemd.md)
  (giữ `KillMode=mixed`, `TimeoutStopSec=660`, `Delegate=yes`), chạy `daemon-reload`, `enable --now`,
  và in lệnh `loginctl enable-linger` nếu chưa bật. `service uninstall`, `service status` đi kèm.
  `--print` chỉ in unit (cho người muốn cài unit hệ thống bằng `sudo`). Không tự tạo user hệ thống.
- Controller crash trên Windows thì không tự bật lại (không có service). Ghi vào backlog nếu cần.

### 3.11 Tự cập nhật

**Phát hành.** Workflow Release thêm file `latest.json` cho mỗi bản: phiên bản, ngày, link ghi chú
phát hành, và cho mỗi đích: URL gói, SHA-256 của binary trong gói. File được ký bằng minisign
(`latest.json.minisig`); khóa bí mật là secret `MINEHOST_MINISIGN_KEY` do chủ dự án tạo, khóa công khai
nhúng trong binary. Bản nháp chưa Publish không bao giờ được thấy vì MineHost đọc
`https://github.com/Pharos107/MineHost/releases/latest/download/latest.json` (chỉ trỏ tới bản đã
Publish, không phải pre-release).

**Kiểm tra.** Mỗi 24 giờ và khi bấm "Kiểm tra ngay". Tắt được trong Cài đặt (mặc định bật; chỉ gửi
một request GET tới GitHub, không kèm dữ liệu nào). Có bản mới thì chấm báo ở thanh trên và mục Cập
nhật hiện phiên bản, ghi chú, nút "Cập nhật".

**Cài.** Chỉ khi người dùng bấm, trong một Job:

1. Tải gói qua `mh-net` (host cho phép: `github.com`, `objects.githubusercontent.com`), kiểm tra chữ ký
   manifest rồi SHA-256 của binary. Sai bất kỳ thứ gì: `UPDATE_SIGNATURE_INVALID`, xóa file.
2. Chạy binary mới với `minehost self-test` (in phiên bản, kiểm tra đọc được thư mục dữ liệu, thoát).
   Không qua thì dừng, không đụng tới bản đang chạy.
3. Hộp thoại xác nhận liệt kê server đang chạy sẽ bị dừng an toàn (ADR 0001). Đồng ý thì
   `shutdown_all()` như nút Thoát.
4. Đổi tên exe đang chạy thành `minehost.old(.exe)` (Windows cho đổi tên exe đang chạy), đặt binary
   mới vào đúng chỗ, ghi `<data>/update.json` `{ from, to, attempts: 0 }`, nhả khóa `minehost.lock`.
5. Khởi động bản mới: chạy dưới systemd thì thoát với mã 75 để systemd bật lại (unit đặt
   `Restart=on-failure`); không thì spawn bản mới tách khỏi process hiện tại (cùng tham số dòng lệnh) rồi
   thoát.

**Quay về.** Ngay đầu `main`, trước khi mở log hay API, binary đọc `update.json`: tăng `attempts`, ghi
lại. Khi đã phục vụ API và bật xong các server `autostart` được 60 giây thì xóa `update.json` (thành
công). Lần khởi động thấy `attempts ≥ 3` thì đổi `minehost.old` về chỗ cũ, ghi lỗi vào
`<data>/update-failed.json`, rồi chạy bản cũ; bản cũ đọc file đó và hiện thông báo "Bản X không chạy
được, đã quay về bản Y". Bản lỗi giữ lại ở `updates/failed/` để gửi báo lỗi.

Dữ liệu: bản mới không được đổi định dạng file theo cách bản cũ không đọc được (tăng `schema` của
`instance.toml` là việc của một spec riêng, có bước chuyển ngược). Ghi thành ADR 0008.

Windows: binary tải bằng MineHost không mang Mark-of-the-Web nên SmartScreen không hỏi lại; binary vẫn
chưa ký code (spec gốc).

### 3.12 Docker: tùy chọn, Windows và Linux

Chủ dự án chọn ngày 2026-11-18: làm trên cả Windows (Docker Desktop) và Linux (Docker Engine). Lợi ích
chính là cô lập: container chỉ thấy thư mục server, mod độc không đọc được file của người dùng (lỗ
hổng A1). Không cài Docker hộ người dùng.

- **Kết nối:** crate `bollard` (Docker Engine API) qua socket mặc định (`/var/run/docker.sock`,
  `//./pipe/docker_engine`) hoặc `DOCKER_HOST`. Gọi được và là Linux container thì lựa chọn Docker
  hiện; không thì ẩn, Cài đặt chung ghi lý do. Docker Desktop đang ở chế độ Windows container cũng ẩn.
- **Image:** `eclipse-temurin:<java_major>-jre` (cùng nhà phát hành với JRE MineHost tải), kéo lần
  đầu trong Job, ghi digest vào `instance.toml`; lần sau dùng đúng digest đó. Nút "Cập nhật image" kéo
  lại và ghi digest mới. Đây là ngoại lệ có chủ đích của "chỉ dùng Java do MineHost tải" (ghi vào ADR
  0007).
- **Chạy:** mount thư mục `server/` vào `/server` (đọc ghi), cwd `/server`, chạy đúng lệnh java như
  cách thường (`args_file` dùng `unix_args.txt`). `--cap-drop ALL`, `no-new-privileges`, root fs chỉ đọc,
  `/tmp` là tmpfs, không mount gì khác. Linux: chạy bằng uid:gid của user đang chạy MineHost để file
  không thành của root. Mạng bridge, publish cổng game và cổng phụ trên mọi địa chỉ như cách thường.
- **Giới hạn:** `memory` theo công thức của P3 (`-Xmx × 1,5 + 1024 MB`, swap bằng 0), `pids_limit`
  2048. Container bị kill vì OOM thì crash có gợi ý `MEMORY_LIMIT` như cách thường.
- **Console, chỉ số:** attach stdin/stdout (không TTY); CPU và RAM từ API stats của Docker.
- **Installer của loader** vẫn chạy bằng Java trên máy (P3), kể cả cho server Docker; file cài ra
  dùng được ở cả hai cách.
- **Chuyển cách chạy:** chỉ khi server tắt; chỉ đổi `[runtime]` trong `instance.toml`, không chép
  world. Backup, trang Tệp, người chơi không đổi gì vì đều làm trên cùng thư mục.

**Vòng đời container (ADR 0007, khác ADR 0001).** Container không chết theo controller. Thoát an toàn
vẫn gửi `stop` và chờ như cách thường. Controller crash thì container chạy tiếp; lần bật sau controller
tìm container theo nhãn `minehost.server-id`, gắn lại stdin/stdout, lấy 5.000 dòng log cuối từ Docker
làm bộ đệm console, và đưa actor vào `Running`. Container của server đã bị xóa khỏi MineHost thì được
dừng (`stop` qua stdin, quá hạn thì kill) rồi xóa. Container tạo với `restart: no` và `auto_remove`
tắt (để đọc mã thoát), MineHost tự xóa sau khi đọc.

**Hiệu năng trên Windows:** thư mục trên ổ NTFS mount vào Linux container của Docker Desktop đi qua lớp
chia sẻ file của WSL2. PR-01 đo với Paper 26.3 ([khảo sát mục 3](p4-khao-sat.md#3-docker-desktop-trên-windows)):

- Khởi động chậm khoảng 2 lần, lưu chunk chậm khoảng 4,5 lần so với cách thường.
- Vượt ngưỡng 3 lần nên UI Windows hiện cảnh báo cạnh lựa chọn Docker.
- Named volume nhanh hơn, nhưng world sẽ nằm ngoài thư mục server nên không dùng; giữ bind mount.

## 4. Mô hình dữ liệu

### 4.1 Thêm vào `instance.toml` (vẫn schema 1)

```toml
[network]                      # mới, tùy chọn; thiếu nghĩa là không mở ra Internet
internet = "playit"            # off | upnp | playit
playit_tunnel = "…"            # id tunnel playit đang dùng cho server này
playit_created = true          # tunnel do MineHost tạo (được phép xóa và tạo lại)
playit_address = "…"           # địa chỉ playit lần cuối, hiện khi agent chưa chạy

[[network.ports]]              # cổng phụ, mục 3.8
name = "Geyser"
protocol = "udp"               # tcp | udp
port = 19132
internet = true

[runtime]                      # mới, tùy chọn; thiếu nghĩa là "process"
kind = "docker"                # process | docker
image = "eclipse-temurin:21-jre"
image_digest = "sha256:…"
```

Bản P3 đọc file này bỏ qua bảng lạ (khóa lạ được giữ) và chạy server bằng process: đúng thư mục, chạy
được. Đây là hành vi mong muốn khi quay về bản cũ.

### 4.2 File mới trong thư mục dữ liệu

| File | Ai ghi | Nội dung | Mất thì sao |
| --- | --- | --- | --- |
| `secrets.toml` (đã có) | `mh-app` | Thêm `playit_secret` | Phải liên kết lại tài khoản playit |
| `tls/cert.pem`, `tls/key.pem` | Binary, lần đầu bật LAN | Chứng chỉ tự ký | Tạo lại; trình duyệt cảnh báo lại một lần |
| `playit/<phiên bản>/playit(.exe)` | `mh-app` | Agent đã kiểm tra SHA-256 | Tải lại |
| `update.json`, `update-failed.json` | Binary | Mục 3.11 | Mất `update.json` giữa chừng thì không quay về tự động được; `minehost.old` vẫn còn để đổi tay |
| `updates/<phiên bản>/` | Job cập nhật | Gói đã tải | Xóa được |
| `config.toml` (đã có) | | Thêm `update_check = true` | Mặc định |

Cập nhật [du-lieu.md](../architecture/du-lieu.md) trong PR thêm từng file.

### 4.3 Mã lỗi mới

`INTERNET_REQUIRES_WHITELIST`, `UPNP_UNAVAILABLE`, `UPNP_CGNAT`, `UPNP_MAPPING_FAILED`,
`PLAYIT_NOT_LINKED`, `PLAYIT_LINK_REJECTED`, `PLAYIT_AGENT_FAILED`, `PLAYIT_TUNNEL_FAILED`,
`PLAYIT_TUNNEL_REJECTED`, `EXTERNAL_CHECK_FAILED`,
`PORT_IN_USE` (đã có, dùng cho cổng phụ), `DOCKER_UNAVAILABLE`, `DOCKER_IMAGE_FAILED`,
`UPDATE_SIGNATURE_INVALID`, `UPDATE_SELF_TEST_FAILED`, `UPDATE_ROLLED_BACK`, `AUTOSTART_FAILED`. Mỗi mã
có bản dịch `vi` và `en`, kèm `params` cần cho câu gợi ý.

## 5. Thiết kế từng crate

Thêm một crate và hai cạnh phụ thuộc; sửa bảng crate trong AGENTS.md cùng PR-02.

| Crate | Thêm | Phụ thuộc mới |
| --- | --- | --- |
| `mh-expose` (mới) | UPnP, nhận CGNAT, client kiểm tra từ ngoài, kế hoạch tải và cấu hình agent playit, đọc log agent | `mh-net`, `mh-core`, `mh-platform` |
| `mh-runtime` | `DockerRuntime`, `enum Runtime { Process, Docker }` như spec gốc | `bollard` (thư viện ngoài) |
| `mh-app` | Use case mở/đóng Internet theo vòng đời server, supervisor agent playit, Job cập nhật, chọn runtime | thêm `mh-expose` |
| `mh-api` | Route mục 5.4 | — |
| `minehost` | TLS LAN, `service`, `self-test`, tự chạy khi đăng nhập, thay binary và quay về | `rcgen`, `tokio-rustls`, `winreg` (Windows), `minisign-verify` |

`mh-expose` theo cách của ADR 0006: nó không chạy process. Agent playit được `mh-app` chạy qua
`mh-runtime`, giống installer của loader.

### 5.1 UPnP (`mh-expose`)

- Tìm IGD bằng SSDP trên các giao diện LAN (`igd-next`, async), hạn 3 giây.
- Ánh xạ TCP (và UDP cho cổng phụ) cổng ngoài = cổng trong tới IP LAN của máy, mô tả
  `MineHost <tên server>`, lease 1 giờ, gia hạn mỗi 30 phút. Router không nhận lease thì dùng 0 (vô hạn)
  và gỡ khi tắt.
- Cổng ngoài đã bị ánh xạ cho máy khác: `UPNP_MAPPING_FAILED` kèm cổng, gợi ý đổi cổng server.
- Controller khởi động thì gỡ các ánh xạ có mô tả `MineHost ` trỏ về máy này mà không ứng với server
  nào đang chạy (dọn sau lần crash).

### 5.2 playit (`mh-expose`, `mh-app`)

- `mh-expose::playit` gồm `PlayitApi`, một client nhỏ trên `mh-net`, và phần dựng lệnh chạy agent.
  - `PlayitApi` gọi `claim/setup`, `claim/exchange`, `v1/agents/rundata`, `v1/tunnels/list`,
    `v1/tunnels/create`, `tunnels/delete` với header `Authorization: Agent-Key <secret>`.
  - Chỉ mô tả các trường MineHost dùng; trường lạ bị bỏ qua để playit thêm trường không làm hỏng.
  - Kiểu lỗi playit trả về (`RequiresPlayitPremium`, `RequiresVerifiedAccount`, …) chuyển thành
    `PLAYIT_TUNNEL_REJECTED` kèm `params.reason`.
- `PlayitAgent` trong `mh-app` là một actor.
  - Trạng thái: `not_installed | needs_link | linking | starting | online | failed`.
  - Bật khi server playit đầu tiên bật, tắt khi server cuối cùng tắt.
  - Liên kết: tạo mã, trả link claim cho UI, hỏi `claim/setup` mỗi 2 giây tới khi `UserAccepted`,
    `UserRejected` hoặc hết 10 phút.
- `PlayitTunnels` trong `mh-app` khớp tunnel với server theo mục 3.4. Nó dò lại `v1/tunnels/list` mỗi
  10 giây tới khi có địa chỉ, sau đó mỗi 5 phút.
- Trạng thái agent và địa chỉ của từng server đi qua bus sự kiện như trạng thái server.
- Cờ `PLAYIT_AUTO_CREATE` là hằng số trong `mh-app`. Tắt thì `PlayitTunnels` chỉ tìm tunnel, không
  tạo; UI hiện hướng dẫn tạo tay.
- `fake-playit`, binary cho test, giả cả API (wiremock) lẫn agent (process in log rồi chờ tín hiệu).

### 5.3 Kiểm tra từ ngoài (`mh-expose`)

`ExternalCheck::probe(addr) -> ProbeResult { reachable, version, motd, cached_until }` gọi
`api.mcstatus.io/v2/status/java/<addr>` với `User-Agent: MineHost/<phiên bản>`, giới hạn 30 giây.
`cached_until` lấy từ `expires_at`. Test bằng wiremock với fixture ghi từ dịch vụ thật (khảo sát mục
2). Mỗi server tối đa một lần mỗi 30 giây.

### 5.4 API

| Route | Việc |
| --- | --- |
| `GET /servers/{id}/network` | Cách mở, trạng thái (đang mở, bị chặn vì whitelist, lỗi), địa chỉ LAN và Internet, cổng phụ |
| `PUT /servers/{id}/network` | Đổi cách mở và cổng phụ; server đang chạy thì áp dụng ngay |
| `POST /servers/{id}/network/check` | Kiểm tra từ ngoài |
| `GET /network/playit`, `POST /network/playit/link`, `DELETE /network/playit` | Trạng thái agent, bắt đầu liên kết (trả link claim), gỡ liên kết |
| `GET /system/docker` | Có Docker không, phiên bản, lý do nếu không |
| `PUT /servers/{id}/runtime` | Đổi cách chạy (server phải tắt); Docker thì trả `202` + Job kéo image |
| `GET /system/update`, `POST /system/update/check`, `POST /system/update/install` | Bản mới, kiểm tra ngay, cài (Job) |
| `GET /system/startup`, `PUT /system/startup` | Khởi động cùng Windows (Linux trả trạng thái unit, chỉ đọc) |

Topic WebSocket `servers` thêm trường `internet` vào trạng thái từng server; topic mới `system` cho
trạng thái agent playit và bản cập nhật.

### 5.5 Supervisor

Không đổi API của actor. Actor nhận thêm sự kiện "đã gắn lại" lúc khởi động (container còn chạy) để
vào thẳng `Running`. Mở/đóng Internet là việc của `mh-app` khi nhận sự kiện trạng thái, không nằm
trong actor.

## 6. Các trang

- **Thẻ Địa chỉ** (Tổng quan server): một dòng cho LAN (như hiện tại), một dòng cho Internet khi công
  tắc bật: địa chỉ, nút chép, chấm trạng thái (đang mở, đang kết nối, bị chặn, lỗi) và một câu lý do.
  Không băng cảnh báo ở đầu trang.
- **Chơi qua Internet** (Cài đặt server): ba lựa chọn Tắt / Tự mở cổng router / Tunnel playit.gg, mỗi
  lựa chọn một câu giải thích từ mục 3.2; lựa chọn gợi ý có nhãn "Gợi ý cho mạng của bạn"; nút "Kiểm
  tra từ Internet" kèm câu về dữ liệu gửi đi; khối hướng dẫn tường lửa Windows; bảng cổng phụ.
- **playit.gg** (Cài đặt chung): trạng thái agent, nút "Liên kết tài khoản" mở trang playit ở tab
  mới, tự cập nhật trạng thái khi liên kết xong, nút "Gỡ liên kết".
- **Cách chạy** (Cài đặt server): Thường / Docker, câu giải thích về cô lập, cảnh báo hiệu năng trên
  Windows (PR-01 đo thấy chậm), digest image và nút cập nhật image.
- **Cập nhật** (Cài đặt chung): phiên bản đang chạy, bản mới, ghi chú, công tắc tự kiểm tra, nút cập
  nhật; hộp thoại xác nhận liệt kê server sẽ dừng.
- **Khởi động cùng máy** (Cài đặt chung): Windows có công tắc; Linux hiện lệnh `minehost service
  install` để chép.

Mọi trang ghép từ `ui/src/ui/`; thiếu kiểu thì thêm `variant`. Bản phác từng trang gửi chủ dự án
xem trước khi code (như các PR giao diện của P3).

## 7. Phụ thuộc mới

| Crate | Thư viện | Giấy phép | Lý do |
| --- | --- | --- | --- |
| `mh-expose` | `igd-next` | MIT | UPnP IGD, có async |
| `mh-runtime` | `bollard` | Apache-2.0 | Docker Engine API, cả socket Unix và named pipe |
| `minehost` | `rcgen` | MIT/Apache-2.0 | Tạo chứng chỉ tự ký |
| `minehost` | `tokio-rustls` | MIT/Apache-2.0 | TLS cho listener LAN (`rustls` đã có) |
| `minehost` | `winreg` | MIT | Khóa `Run` không cần `unsafe` |
| `minehost` | `minisign-verify` | MIT | Kiểm tra chữ ký manifest |

`cargo-deny` phải xanh với từng thư viện; giấy phép kiểm lại lúc thêm.

## 8. Bảo mật

Trả lời năm câu của [bao-mat.md](../architecture/bao-mat.md#khi-thêm-tính-năng-mới) cho P4:

- **Mở cổng:** P4 lần đầu đưa cổng game ra Internet. Bảo vệ: mục 3.6; server Minecraft tự xác thực
  người chơi khi `online-mode=true`; không mở cổng 7878 (mục 3.1). Chống DDoS: dùng tunnel che IP thay
  vì tự chống (backlog, "đã cân nhắc").
- **Agent playit** là binary bên ngoài chạy với quyền của user: chỉ bản ghim hash, tải từ GitHub
  Releases của playit qua danh sách host. Secret ở `secrets.toml` (Linux `600`); spec gốc muốn
  keyring của OS, để backlog vì cần thêm thư viện và cách xử lý khi không có keyring.
- **Kiểm tra từ ngoài** gửi IP công khai tới dịch vụ bên thứ ba: chỉ khi người dùng bấm, có câu báo.
- **TLS:** khóa riêng không ra khỏi `tls/`; không có trong backup, API. Dấu vân tay chỉ hiện từ
  `127.0.0.1` (mục 3.7).
- **Tự cập nhật** là đường chạy code mới với quyền của user: chữ ký minisign trên manifest, SHA-256
  trên binary, URL cố định, chỉ bản đã Publish. Mất khóa bí mật thì xoay khóa bằng một bản phát hành
  ký bằng khóa cũ chứa khóa công khai mới. Ghi cách giữ khóa trong [ops/phat-hanh.md](../ops/phat-hanh.md).
- **Docker** giảm A1 khi bật: container không thấy file ngoài thư mục server, không capability, không
  leo quyền. Socket Docker tương đương root trên Linux; MineHost chỉ dùng chứ không tự thêm user vào
  nhóm `docker`. Cập nhật A1, A3 trong bao-mat.md khi xong nhóm tương ứng.

## 9. Test

| Lớp | Nội dung chính |
| --- | --- |
| Unit | Bảng mục 3.6; nhận CGNAT từ IP; đọc `[network]`, `[runtime]` giữ khóa lạ; phân tích `latest.json`, chữ ký đúng/sai (khóa thử); đếm `attempts` và quyết định quay về |
| `mh-expose` | IGD giả (SSDP + SOAP trên `127.0.0.1`): tìm, ánh xạ, gia hạn, gỡ, lỗi cổng đã dùng; dịch vụ kiểm tra giả bằng wiremock |
| App (`fake-mc`) | Bật server có `internet = "upnp"` thì ánh xạ, crash thì gỡ; whitelist tắt khi đang mở thì đóng; agent playit giả (binary nhỏ trong repo như `fake-mc`, in mã claim, chết theo kịch bản) được bật lại có giới hạn |
| Runtime Docker | Trên Linux CI (runner GitHub có Docker): tạo container với `fake-mc` trong image nhỏ, console, kill, OOM, stats, gắn lại sau khi drop runtime; bỏ qua khi không có Docker |
| Binary | Listener LAN: TLS bắt tay được, HTTP nhận `308`, cookie `Secure`; `service install --print` ra unit đúng mẫu; `self-test`; cập nhật A → B hỏng → quay về A (hai bản `minehost` dựng với phiên bản khác nhau qua biến build) |
| Tích hợp (`integration`) | Paper thật Process → Docker → Process trên Linux; Windows chỉ cách thường |
| E2E (Playwright) | Bật Internet bị chặn vì whitelist rồi bật whitelist; chọn playit với agent giả tới khi có địa chỉ; hộp thoại cập nhật với manifest giả; đổi cách chạy (Docker giả ở chế độ e2e) |
| Thử tay | Mục 1 tiêu chí 1, 2 và Docker Desktop trên Windows; ghi ở `p4-ket-qua-thu-tay.md` |

Test thường không gọi mạng thật; test gọi playit, dịch vụ kiểm tra hay Docker Hub thật đánh dấu
`#[ignore]`.

## 10. Rủi ro

| Rủi ro | Cách giảm |
| --- | --- |
| playit đổi agent, API hay điều khoản; tài khoản miễn phí bị giới hạn thêm | Ghim phiên bản agent; client chỉ đọc trường cần dùng; cờ `PLAYIT_AUTO_CREATE` tắt được bước tạo tunnel; UPnP vẫn chạy độc lập |
| playit không đồng ý cho tạo tunnel tự động (điều khoản mục 3.4) | Hỏi trước khi phát hành (khảo sát mục 1.7); tắt cờ, người dùng tạo tunnel trên web, MineHost tự nhận |
| Agent playit chết lặp (đã thấy trên dangserver tháng 10) | Supervisor có giới hạn bật lại, trạng thái `failed` hiện rõ kèm 40 dòng log cuối |
| Router báo UPnP thành công mà không chuyển gói | Không bao giờ coi ánh xạ thành công là "vào được"; chỉ nút kiểm tra từ ngoài mới cho kết luận |
| Dịch vụ kiểm tra từ ngoài ngừng hoặc cache lâu | Ghi rõ `cached_until`; đổi dịch vụ là một PR nhỏ vì nằm sau một trait |
| Người dùng mất quyền vào giao diện qua LAN vì cảnh báo chứng chỉ | Hướng dẫn cụ thể từng trình duyệt; dấu vân tay để so; `127.0.0.1` vẫn HTTP |
| Bản cập nhật lỗi làm hỏng dữ liệu, quay về rồi vẫn hỏng | Không đổi định dạng file trong bản cập nhật (ADR 0008); `self-test` trước khi thay |
| Mất hoặc lộ khóa ký bản phát hành | Khóa chỉ ở secret GitHub và một bản sao ngoại tuyến của chủ dự án; quy trình xoay khóa mục 8 |
| Docker Desktop trên Windows chậm hoặc khó dùng | Tùy chọn, mặc định vẫn là cách thường; đã đo ở PR-01 (lưu chunk chậm khoảng 4,5 lần), cảnh báo trong UI |
| Container gắn lại sai server | Nhãn chứa id server và đường dẫn mount; khớp cả hai mới gắn |
| Phạm vi P4 lớn | Sáu nhóm, hai bản phát hành (mục 11) |

## 11. Phát hành

Đề xuất hai bản:

- `v0.6.0` sau nhóm A–D: Internet (UPnP, playit, kiểm tra, chặn whitelist), HTTPS LAN, khởi động cùng
  máy, tự cập nhật. Đây là bản đầu tiên có tự cập nhật nên phải tải tay lần cuối.
- `v0.7.0` sau nhóm E–F: Docker, cổng phụ. Người dùng `v0.6.0` nhận qua tự cập nhật, đồng thời là lần
  thử thật đầu tiên của luồng cập nhật. Xong bản này thì P4 xong.

## 12. Cần chủ dự án quyết định khi duyệt

1. Hai bản phát hành như mục 11, hay một bản `v0.6.0` khi xong cả P4.
2. Tạo cặp khóa minisign và secret `MINEHOST_MINISIGN_KEY` (PR-14 ghi lệnh), giữ một bản khóa ngoại
   tuyến.
3. Chấp nhận dùng dịch vụ công khai cho "Kiểm tra từ Internet" (mục 3.5) hay bỏ nút này.
4. Thứ tự: đặt Docker (nhóm E) sau tự cập nhật như kế hoạch, hay làm sớm hơn.
5. Lần thử tay 5 phút của P1 trên Windows mới cài vẫn còn mở: làm trước nhóm B, hay gộp vào lần thử
   tay của P4 (người chơi ở mạng khác vào server vừa tạo).
6. Gửi câu hỏi ở [khảo sát mục 1.7](p4-khao-sat.md#17-câu-hỏi-gửi-playit-chủ-dự-án-gửi) cho playit và
   quyết định bật cờ `PLAYIT_AUTO_CREATE` khi phát hành nếu chưa có trả lời.
