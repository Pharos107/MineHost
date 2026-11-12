# Bảo mật: mô hình mối đe dọa

- Khớp code tới: 2026-11-12 (sau khi merge #44). Đã có lưu mật khẩu admin; phiên, CSRF và LAN mô tả
  theo spec P2 vì chưa merge.
- Lỗ hổng đã biết được đánh số theo [rà soát 2026-11-11](../ra-soat/2026-11-11-kien-truc.md).

## Cần bảo vệ gì

1. **Máy của người host**: file cá nhân, tài khoản, các chương trình khác. Đây là tài sản quan trọng
   nhất, vì người dùng chính chạy MineHost trên máy tính cá nhân.
2. **World và cấu hình server**: không mất, không bị sửa ngoài ý muốn.
3. **Quyền điều khiển MineHost**: ai gửi được lệnh console hoặc tải file lên thì chạy được code trên
   máy host (xem "Quyền admin nghĩa là gì").

## Ai có thể tấn công

| Tác nhân | Vào bằng đường nào | Mức tin cậy |
| --- | --- | --- |
| Trang web người dùng đang mở trong trình duyệt | Gửi request tới `127.0.0.1:7878`, DNS rebinding | Không tin |
| Chương trình hoặc user khác trên cùng máy | Kết nối thẳng tới `127.0.0.1:7878` | **Đang được tin hoàn toàn** khi chưa đặt mật khẩu |
| Máy khác trong LAN (khi bật LAN, P2) | Giao diện web qua HTTP; nghe lén Wi-Fi | Không tin cho tới khi đăng nhập |
| Người chơi trong server | Tên, chat, lệnh game; nội dung này đi vào log mà MineHost phân tích | Không tin |
| Plugin, mod, jar server | Chạy trong process Java, cùng user với controller | **Đang được tin hoàn toàn** |
| Máy chủ tải về (Mojang, Paper, Adoptium) và đường truyền | File và JSON trả về | Tin sau khi kiểm tra hash |
| File người dùng đưa vào (upload, backup để khôi phục) | Tên file, đường dẫn trong archive | Không tin |

## Lớp bảo vệ đang có

| Mối đe dọa | Lớp bảo vệ | Ở đâu |
| --- | --- | --- |
| Trang web lạ gọi API (CSRF, DNS rebinding) | `Host` phải là `127.0.0.1:<cổng>` hoặc `localhost:<cổng>`; request thay đổi dữ liệu và WebSocket phải có `Origin` của MineHost, hoặc header `X-MineHost: 1` khi không có `Origin`; body phải là JSON (trừ route upload) | `mh-api/src/guard.rs` |
| Thoát khỏi thư mục server qua đường dẫn | `SafeRoot::join`: chặn `..`, `:`, ký tự điều khiển, tên thiết bị Windows, symlink trỏ ra ngoài | `mh-platform/src/safe_path.rs` |
| Archive độc (zip-slip, symlink) | Giải nén JRE và backup đi qua `SafeRoot`; bỏ symlink, hardlink, file thiết bị; backup kiểm tra sha256 trước khi giải nén | `mh-java/src/extract.rs`, `mh-backup/src/restore.rs` |
| File tải về bị sửa | Kiểm tra sha1 hoặc sha256 trong lúc tải; không khớp thì xóa | `mh-net/src/download.rs` |
| Chèn lệnh console qua tên người chơi hoặc tin nhắn | Tên phải khớp `[A-Za-z0-9_]{3,16}`; tin nhắn được escape JSON; lệnh console không được chứa ký tự xuống dòng | `mh-core/src/players/`, `mh-runtime/src/lib.rs` |
| Chat giả dòng "joined the game" | Bỏ tiền tố log nếu nhận ra (`[hh:mm:ss INFO]: ` hoặc `[hh:mm:ss] [luồng/INFO]: `), rồi đòi **toàn bộ** phần trước ` joined the game` phải là một tên hợp lệ. Dòng chat `<Steve> Notch joined the game` vì vậy bị loại. Tiền tố không bắt buộc: dòng không có tiền tố vẫn được phân tích nguyên dòng | `mh-core/src/players/log.rs` |
| Tự ý đồng ý EULA | Chỉ ghi `eula=true` sau khi người dùng bấm đồng ý | `mh-app/src/launcher.rs` |
| Phụ thuộc có lỗ hổng hoặc giấy phép lạ | `cargo-deny` trong CI | `deny.toml` |
| Lỗi bộ nhớ | `unsafe` bị cấm ngoài ba module nhỏ; cấm `unwrap`, `expect`, `panic!` | `Cargo.toml`, ADR 0002 |
| Ghi đè `server.jar` khi server đang chạy | Chặn sửa và xóa | `mh-app/src/files.rs` |

Sẽ có khi nhóm G của P2 merge: mật khẩu băm `argon2id`, cookie phiên `HttpOnly` và `SameSite=Strict`,
`X-CSRF-Token`, giới hạn đăng nhập sai theo IP ([spec P2 mục 8](../specs/p2-nhieu-server-va-van-hanh.md#8-bảo-mật)).

## Quyền admin nghĩa là gì

Người điều khiển được MineHost thì làm được ba việc, mỗi việc đều dẫn tới chạy code tùy ý với quyền
của user đang chạy MineHost:

- gửi lệnh console (quyền như op);
- tải một file `.jar` vào `plugins/` rồi khởi động lại server;
- sửa `server.properties` và các file cấu hình khác.

Vì vậy mật khẩu admin không chỉ bảo vệ server Minecraft mà bảo vệ cả máy host. Mọi quyết định về
đăng nhập, LAN và truy cập từ Internet phải xét theo mức đó.

## Lỗ hổng đã biết

| Mã | Lỗ hổng | Mức | Trạng thái |
| --- | --- | --- | --- |
| A1 | Process server không bị cô lập: cùng user với controller, đọc được mọi file của user (kể cả `auth.toml` sau này), không giới hạn cứng RAM hay CPU. Plugin hoặc mod độc là mất máy | Cao | Chưa có kế hoạch trước Docker (P4, tùy chọn) |
| A3 | Bật LAN thì mật khẩu admin đi qua HTTP không mã hóa; ai nghe được Wi-Fi là có quyền admin | Cao | Đã duyệt là chấp nhận ở P2, TLS ở P4; đang đề nghị xem lại |
| C1 | Chưa đặt mật khẩu thì mọi chương trình và mọi user trên cùng máy điều khiển được MineHost. Trên máy Linux nhiều người dùng, đây là đường leo quyền sang user chạy MineHost | Trung bình | Đặt mật khẩu (P2) thì hết; mặc định vẫn là không mật khẩu |
| C2 | `SafeRoot` kiểm tra rồi mới dùng: giữa hai bước, một plugin có thể đổi thư mục thành symlink trỏ ra ngoài | Thấp | Chỉ khai thác được bởi code đã chạy trong server, vốn đã có quyền đó (A1) |
| C3 | Giao diện không gửi header `Content-Security-Policy` hay `X-Frame-Options` | Thấp | Chưa có kế hoạch |
| C4 | Binary Windows chưa ký; bản phát hành chỉ có `.sha256` cùng chỗ với file, không có chữ ký | Thấp | Đã chốt chưa mua chứng chỉ (spec gốc) |
| C5 | Controller không kiểm tra việc bị chạy dưới root hoặc Administrator | Thấp | Chưa có kế hoạch |

Không nằm trong phạm vi hiện tại (đã quyết định, không phải lỗ hổng): nhiều tài khoản và phân quyền,
2FA, API token, nhật ký thao tác (P5); chống DDoS (server chỉ mở trong LAN cho tới P4).

## Khi thêm tính năng mới

Trước khi merge, trả lời được các câu sau trong PR:

1. Có đường dẫn, tên file hay archive nào đến từ người dùng hoặc từ mạng không? Nếu có, nó đi qua
   `SafeRoot` ở đâu?
2. Có chuỗi nào từ người chơi hoặc từ log được ghép vào lệnh console không?
3. Có route mới nào nhận body không phải JSON không? Nó có còn qua kiểm tra `Origin` không?
4. Có file nào tải từ mạng mà không có hash để kiểm tra không?
5. Tính năng có mở thêm cổng hoặc lắng nghe ngoài `127.0.0.1` không?
