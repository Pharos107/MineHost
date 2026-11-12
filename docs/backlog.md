# Backlog

Ý tưởng nằm ngoài giai đoạn hiện tại. Ghi vào đây thay vì code (quy tắc trong [AGENTS.md](../AGENTS.md)).
Một dòng ở đây không phải là cam kết: muốn làm thì chọn vào một giai đoạn và viết spec trong `docs/specs/`.

Lỗi của giai đoạn đã xong và vấn đề mở của spec đang làm **không** ghi ở đây; chúng nằm ở
[rà soát 2026-11-11](ra-soat/2026-11-11-kien-truc.md) và mục 12 của spec P2.

## Cách ghi

Mỗi dòng: ý tưởng, vì sao đáng làm, nguồn (ai đề xuất hoặc tài liệu nào), giai đoạn gợi ý. Dòng nào
được nhận vào spec thì xóa khỏi đây và dẫn link spec trong PR.

## An toàn dữ liệu và cô lập

| Ý tưởng | Vì sao | Nguồn | Gợi ý |
| --- | --- | --- | --- |
| Cô lập process server không cần Docker: Job Object giới hạn RAM và số process (Windows); Landlock và cgroup v2 (Linux) | Plugin, mod đang chạy với toàn quyền của user. Pterodactyl bắt buộc Docker, Crafty chạy trần: đây là chỗ MineHost có thể đi trước. Lưu ý: Job Object chỉ giới hạn tài nguyên, **không** chặn mod đọc file của user (mã độc kiểu Fractureiser chạy ngay trong JVM). Chặn file trên Windows cần AppContainer hoặc token mức thấp, chưa rõ có chạy được với JVM không; trên Linux thì Landlock làm được | Rà soát A1 | Trước P3 |
| Thư mục backup thứ hai (ổ khác, NAS, thư mục đồng bộ đám mây) | Backup cùng ổ với world thì hỏng ổ là mất cả hai. Dùng thư mục đồng bộ sẵn có nên không cần tích hợp API đám mây nào | Rà soát A4 | P3 |
| Backup lưu theo hash từng file | File region ít thay đổi; tránh chép lại cả world mỗi lần | Rà soát A4 | P3 trở đi |
| Tự kiểm tra backup định kỳ (hash, mở được `level.dat`) | Backup chỉ có giá trị khi khôi phục được | Rà soát, phần lợi thế | P3 |
| Đích backup S3, Backblaze B2, Google Drive | Chuẩn của các panel khác | Yêu cầu rà soát | P5, sau thư mục thứ hai |
| Dọn `trash/` theo tuổi hoặc dung lượng | `trash/` hiện chỉ lớn lên | [du-lieu.md](architecture/du-lieu.md) | P3 |

## Hiệu năng và chẩn đoán

| Ý tưởng | Vì sao | Nguồn | Gợi ý |
| --- | --- | --- | --- |
| Preset cờ JVM (Aikar cho Paper, ZGC khi nhiều RAM), `Xms = Xmx` | Hiện chỉ có `-Xms`, `-Xmx`. Chọn cờ theo major: `-XX:+ZGenerational` chỉ cần ở Java 21–22, từ Java 23 là mặc định. `Xms = Xmx` giữ chặt RAM cả khi không ai chơi, nên để người dùng tắt được trên máy cá nhân | Rà soát B2 | P3 |
| TPS và MSPT trên trang Tổng quan, cảnh báo khi lag | "Server lag" là phàn nàn thường gặp nhất; spec P2 đã loại khỏi P2 | Rà soát B3 | P3 |
| "Bác sĩ server": đọc `crash-reports/` và `hs_err_pid*.log`, chỉ ra mod gây lỗi, thiếu dependency, sai Java, kèm nút sửa | Khung `ErrorCode` kèm gợi ý sửa đã có; các panel khác chỉ đưa log thô | Rà soát, phần lợi thế | P3 (cùng mod) |
| Tích hợp Spark profiler | Chuẩn chẩn đoán lag của cộng đồng | Yêu cầu rà soát | P5 |
| Lịch sử chỉ số dài hơn 1 giờ | Hiện mất khi tắt controller | Spec P2, "không làm" | Cùng lúc với SQLite (P5) |
| Cập nhật build Paper cho server đã tạo, có backup trước | Bản vá bảo mật không tới server cũ | Rà soát B4 | P2 hoặc đầu P3 |
| Chọn Java riêng cho từng server trong giao diện | Modpack cũ đôi khi cần đúng một bản Java | Rà soát B1 | P3 |
| Cách chạy server ngoài `-jar`: Forge và NeoForge từ 1.17 chạy bằng `java @user_jvm_args.txt @libraries/…/<os>_args.txt`, không qua `-jar server.jar` | `JavaLaunch` và `instance.toml` hiện chỉ có `jar`. Thêm được một bảng `launch` có mặc định là `jar` mà vẫn ở schema 1 (khóa lạ được giữ). Forge bản mới có thể có shim jar để chạy bằng `-jar`; cần kiểm tra | Đánh giá bên ngoài (2026-11-12) | Quyết định trong spec P3, trước khi code loader |

## Mạng và người chơi

| Ý tưởng | Vì sao | Nguồn | Gợi ý |
| --- | --- | --- | --- |
| "Mời bạn bằng một đường link": tunnel tích hợp kèm chẩn đoán tường lửa, CGNAT, kiểm tra cổng từ ngoài | Nhóm bạn hiếm khi chung LAN; các panel khác để người dùng tự lo mạng | Spec gốc P4; rà soát B6 | P4, xét đưa lên trước P3 |
| Thêm lựa chọn tunnel: Cloudflare Tunnel, Tailscale, ngrok | Spec gốc chỉ có playit.gg và UPnP | Yêu cầu rà soát | P4 |
| Chặn mở ra Internet khi `online-mode=false` mà không bật whitelist | Ai cũng vào được bằng tên của op | Rà soát | P4 |
| Geyser và Floodgate một cú bấm (người chơi Bedrock, điện thoại) | Phù hợp nhóm người dùng tiếng Việt. Là phán đoán, chưa có số liệu | Rà soát, phần lợi thế | Sau P3 |
| Hướng dẫn an toàn cho server offline-mode | Offline-mode phổ biến; rủi ro mạo danh | Rà soát, phần lợi thế | Sau P2 |
| Cấp và kiểm tra cổng phụ (UDP của Geyser, voice chat, bản đồ web) | Hiện chỉ quản lý cổng game | Rà soát C6 | P3, P4 |
| TLS cho giao diện qua LAN | Mật khẩu admin đang đi qua HTTP | Spec P2 mục 3.4; rà soát A3 | Đang đề nghị kéo về P2 |

## Từ spec gốc (P5)

Nhiều tài khoản và phân quyền theo server; nhật ký thao tác; API token; 2FA; MCP để AI điều khiển
server; proxy Velocity; Prometheus metrics; telemetry có hỏi ý kiến; thêm ngôn ngữ; Hangar và SpigotMC
làm nguồn plugin bên cạnh Modrinth.

## Đã cân nhắc và không làm

| Ý tưởng | Lý do |
| --- | --- |
| Nhiều node, một bảng điều khiển cho nhiều máy | Ngoài phạm vi v2.0 (spec gốc). Không phải thị trường của MineHost |
| Host thương mại, hạn mức theo gói | Ngoài phạm vi v2.0 (spec gốc) |
| Tự xây lớp chống DDoS tầng 7 | Khi mở ra Internet thì dùng tunnel che IP thay vì tự chống |
| Bedrock Dedicated Server | Ngoài phạm vi v2.0; Geyser là đường ngắn hơn |
