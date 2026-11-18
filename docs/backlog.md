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
| Cô lập process server không cần Docker: Job Object giới hạn RAM và số process (Windows); Landlock và cgroup v2 (Linux) | Plugin, mod đang chạy với toàn quyền của user. Pterodactyl bắt buộc Docker, Crafty chạy trần: đây là chỗ MineHost có thể đi trước. Lưu ý: Job Object chỉ giới hạn tài nguyên, **không** chặn mod đọc file của user (mã độc kiểu Fractureiser chạy ngay trong JVM). Chặn file trên Windows cần AppContainer hoặc token mức thấp, chưa rõ có chạy được với JVM không; trên Linux thì Landlock làm được | Rà soát A1 | Giới hạn RAM và số process đã vào [spec P3](specs/p3-mod-va-modpack.md#38-giới-hạn-tài-nguyên-của-process-server-rà-soát-a1-phần-đầu); còn lại chặn đọc file: sau P3 |
| Thư mục backup thứ hai (ổ khác, NAS, thư mục đồng bộ đám mây) | Backup cùng ổ với world thì hỏng ổ là mất cả hai. Dùng thư mục đồng bộ sẵn có nên không cần tích hợp API đám mây nào | Rà soát A4 | Sau P3, chưa xếp giai đoạn |
| Backup lưu theo hash từng file | File region ít thay đổi; tránh chép lại cả world mỗi lần | Rà soát A4 | Sau P3, chưa xếp giai đoạn |
| Tự kiểm tra backup định kỳ (hash, mở được `level.dat`) | Backup chỉ có giá trị khi khôi phục được | Rà soát, phần lợi thế | Sau P3, chưa xếp giai đoạn |
| Đích backup S3, Backblaze B2, Google Drive | Chuẩn của các panel khác | Yêu cầu rà soát | P5, sau thư mục thứ hai |
| Dọn `trash/` tự động theo tuổi hoặc dung lượng (dọn tay đã vào [spec nâng cấp trang Tệp](specs/p3-nang-cap-trang-tep.md)) | `trash/` hiện chỉ lớn lên | [du-lieu.md](architecture/du-lieu.md) | Sau P3, chưa xếp giai đoạn |
| Quét bytecode jar mod và plugin tìm dấu hiệu mã độc (`Runtime.exec`, `URLClassLoader` tải từ mạng, đọc file trình duyệt hay ví, kiểu Fractureiser) trước khi bật | Bổ sung cho dòng cô lập process ở trên: Job Object không chặn mod đọc file của user. Quét tĩnh hợp với bảo mật hơn hiệu năng (cảnh báo hiệu năng tĩnh nhiều báo động giả, JFR làm tốt hơn). Ý tưởng chưa kiểm tra: chưa đo tỉ lệ báo động giả trên modpack thật | Thảo luận 2026-11-18, chưa kiểm tra | Sau v0.5.0 |

## Hiệu năng và chẩn đoán

| Ý tưởng | Vì sao | Nguồn | Gợi ý |
| --- | --- | --- | --- |
| Preset cờ JVM (Aikar cho Paper, ZGC khi nhiều RAM), `Xms = Xmx` | Hiện chỉ có `-Xms`, `-Xmx`. Chọn cờ theo major: `-XX:+ZGenerational` chỉ cần ở Java 21–22, từ Java 23 là mặc định. `Xms = Xmx` giữ chặt RAM cả khi không ai chơi, nên để người dùng tắt được trên máy cá nhân | Rà soát B2 | Sau `v0.5.0` (quyết định 2026-11-18), chưa xếp giai đoạn |
| TPS và MSPT trên trang Tổng quan, cảnh báo khi lag | "Server lag" là phàn nàn thường gặp nhất; spec P2 đã loại khỏi P2 | Rà soát B3 | Sau P3, chưa xếp giai đoạn |
| "Bác sĩ server": đọc `crash-reports/` và `hs_err_pid*.log`, chỉ ra mod gây lỗi, thiếu dependency, sai Java, kèm nút sửa | Khung `ErrorCode` kèm gợi ý sửa đã có; các panel khác chỉ đưa log thô | Rà soát, phần lợi thế | Sau P3, chưa xếp giai đoạn |
| Tích hợp Spark profiler | Chuẩn chẩn đoán lag của cộng đồng | Yêu cầu rà soát | P5 |
| Lịch sử chỉ số dài hơn 1 giờ | Đã có lưu đa tầng trong RAM (tới 9.999 ngày, xem [spec nâng cấp biểu đồ](specs/nang-cap-bieu-do.md)), nhưng tắt controller vẫn mất; cần chuyển phần lưu giữ sang SQLite để giữ bền vững qua lần tắt máy | Spec P2, "không làm"; nâng cấp biểu đồ | Cùng lúc với SQLite (P5) |

Các dòng dưới đây là **ý tưởng chưa kiểm tra**: chưa đo, chưa làm nguyên mẫu, chưa khảo sát kỹ các dự án đã có
(Spark, Observable, ServerCore, Pufferfish). Nhận dòng nào vào spec thì phải thử trước điều được ghi là "cần đo"
hoặc "cần kiểm tra". Hai điều đã thấy tận mắt (2026-11-18): Java do MineHost tải là JRE, có `jfr` và module
`jdk.jfr` nhưng không có `jstat`, `jcmd`; jar Minecraft 1.20.1 có sẵn sự kiện JFR `ServerTickTimeEvent`,
`ChunkGenerationEvent`, `PacketSent`/`PacketReceived`, bật bằng lệnh `/jfr start`.

| Ý tưởng | Vì sao | Nguồn | Gợi ý |
| --- | --- | --- | --- |
| Sức khỏe heap sau GC: bật log GC (`-Xlog:gc:file=...`, xoay vòng), controller đọc heap còn lại sau GC; khi lượng object sống tiến gần `-Xmx` và % thời gian GC cao thì cảnh báo "RAM sắp không đủ", tùy chọn khởi động lại an toàn lúc không có người chơi | Phân biệt RAM dao động răng cưa với RAM bị giữ chặt, báo trước khi server tụt TPS vì GC chạy liên tục. Không cần agent, JMX hay `jstat`. Thước đo phải theo GC: với G1 chỉ tính heap sau mixed GC, full GC hoặc sau chu kỳ đánh dấu (sau young GC Old Gen tăng dần là bình thường); ZGC tính khác; Java 8 có cờ và định dạng log riêng. Không gọi là "rò rỉ" vì object sống tăng hợp lệ theo người chơi, chunk, cache. Nhận ra `OutOfMemoryError` trong log để chuyển Crashed thay vì treo. Cùng dữ liệu cho biết heap quá to hay quá nhỏ (đầu vào cho preset JVM) | Thảo luận 2026-11-18, chưa kiểm tra | Sau v0.5.0, làm trước các dòng dưới |
| Controller điều phối process và thread của server: (1) trên Windows gọi `SetProcessInformation(ProcessPowerThrottling)` để server chạy ẩn không bị coi là process nền (EcoQoS, E-core); (2) CPU Sets mềm (`SetThreadSelectedCpuSets`) đặt "Server thread" vào nhóm P-core, hoặc CCD có V-Cache trên Ryzen X3D; (3) hạ ưu tiên CPU rồi I/O cho việc nền của MineHost (nén backup) | Rẻ, không đụng logic game; nằm cạnh Job Object trong `mh-runtime/src/os/`. Cần đo riêng từng phần (chỉ (1), (1)+(2), có/không (3)) bằng MSPT P95/P99 trên CPU hybrid. (2) có thể thừa vì Thread Director tự xếp khi hết EcoQoS; V-Cache tốt hơn là giả thuyết, tách Server thread khỏi thread sinh chunk qua hai CCD có thể hại; cần cách tìm "Server thread" trên Windows (JRE không có `jcmd`; thử `osThreadId` trong JFR hoặc thread tốn CPU nhất); Linux chỉ có ghim cứng nên (2) gần như chỉ cho Windows. (3): backup đã `save-off` nên không tranh với autosave mà tranh CPU nén và đọc region; chạy trên thread riêng vì thread của `spawn_blocking` được dùng lại; `ioprio` trên Linux chỉ có tác dụng với BFQ (NVMe mặc định `none`). Không hạ thread sinh chunk và thread GC lúc dừng thế giới; Linux chỉ hạ, không tăng; (1) trên laptop chạy pin phải có công tắc | Thảo luận 2026-11-18, chưa kiểm tra | Sau v0.5.0 |
| X-ray lag theo yêu cầu và cầu dao trên UI: khi MSPT cao kéo dài hoặc người dùng bấm, controller gửi lệnh vanilla qua console trong 15–30 giây (`/jfr` cho MSPT và tọa độ chunk mới sinh, `/debug start`/`stop` cho thời gian theo loại entity, block entity), hiện kết quả kiểu "`create:mechanical_crafter` chiếm 34% tick"; chủ server bấm hành động tường minh | Không bóp tick ngầm (tránh lặp lại `max-tick-time` của Spigot mà Paper đã tắt), không cần agent, chạy với mọi loader có code vanilla. Hành động: dọn item tại một chunk bằng `kill @e[type=item,...]` (hiện số lượng trước, ghi lại lệnh); "đóng băng" vùng bằng cách gỡ ticket force-load để chunk tự dỡ, vì mod nào cũng chịu được chunk bị dỡ, còn bỏ tick một vùng thì hỏng máy vắt qua mép chunk; có người chơi đứng gần thì báo, không ép; luôn tạm thời và có hoàn tác. Cần kiểm tra: sự kiện JFR của Minecraft có phát khi bật bằng `-XX:StartFlightRecording` không; Paper có làm rỗng profiler `/debug` không (Paper có Spark sẵn); vanilla cho loại chứ không cho tọa độ từng cụm máy, cần tọa độ thì mới tính hook (luôn chèn, kiểm tra cờ `volatile`, không gỡ bằng `retransformClasses`) | Thảo luận 2026-11-18, chưa kiểm tra | Sau v0.5.0, bước đầu của bản đồ chi phí |
| Bản đồ chi phí tick theo từng mod, luôn bật: JFR ghi mẫu stack, controller ghép với bảng "class → jar" dựng từ lần quét mod (`content.toml`), UI hiện mod nào chiếm bao nhiêu % "Server thread", cấp phát nhiều, chờ lock, chặn tick bằng I/O (`jdk.SocketRead`, `jdk.FileWrite` trên "Server thread"); tự chụp khi lag spike | Người dùng không đọc được flame graph; chỗ mới là chạy với mọi loader, không phải cài gì, nối thẳng vào trang Mod (đo theo mod đã có ở Spark, Observable). JFR bắt I/O chặn tick chính xác hơn quét bytecode tĩnh. Cần giải: code Mixin bị trộn vào class Minecraft nên phải đọc metadata Mixin; luật quy chi phí cho frame đầu tiên không thuộc vanilla/loader; JFR lấy mẫu thưa nên chỉ cho tỉ lệ qua vài phút; Java 8 chỉ đọc file dump; "cấp phát nhiều" khác "gây pause dài". Agent chỉ thêm nếu bảng class → jar mập mờ quá nhiều. Nút tắt mod phải kèm backup vì block, item của mod mất khỏi world. Nghiệm thu: jar thử tốn X ms/tick được quy đúng X ms | Thảo luận 2026-11-18, chưa kiểm tra | Sau X-ray; thay dòng Spark ở trên nếu nhận |
| Chạy đo tải trên bản sao: dựng world tạm từ backup (không mở cổng), force-load các vùng nặng nhất mà X-ray hoặc bản đồ chi phí tìm ra, khởi động trước rồi đo nhiều lượt, so MSPT, P99, GC giữa cấu hình cũ và mới | Chứng minh bằng số đo trước khi áp thay đổi vào server thật (cờ JVM, đổi GC, thêm mod tối ưu, bỏ thử một mod, `-XX:ActiveProcessorCount=N` để mọi pool dựng theo số nhân cùng nhỏ lại thay vì sửa config từng mod). Không phát lại gói tin vì Minecraft không tất định và khó giả bắt tay Forge/NeoForge. Giới hạn: chỉ đo tải nền, không đo lag do người chơi (spawn, AI, gửi chunk); tốn thêm RAM, CPU nên chỉ chạy khi máy đủ và server thật vắng; Forge cũ cần cờ xác nhận khi thiếu registry | Thảo luận 2026-11-18, chưa kiểm tra | Sau X-ray |
| Kiểm tra mod tối ưu thiếu hoặc xung đột theo loader và phiên bản, dựa trên danh sách mod đã quét | Gợi ý cụ thể cho server người dùng tự ghép mod. Dùng trước dữ liệu mod tự khai (`breaks`/`conflicts` của Fabric, `incompatible` trong `mods.toml`, dependency `incompatible` của Modrinth), bảng luật tự soạn chỉ bù chỗ thiếu và mỗi luật ghi ngày kiểm tra (ví dụ cũ nhanh: Starlight đã thừa từ khi vanilla viết lại ánh sáng). Không gợi ý thêm mod vào modpack có sẵn vì tác giả pack có thể cố ý bỏ | Thảo luận 2026-11-18, chưa kiểm tra | Sau v0.5.0 |

## Mạng và người chơi

| Ý tưởng | Vì sao | Nguồn | Gợi ý |
| --- | --- | --- | --- |
| Geyser và Floodgate một cú bấm (người chơi Bedrock, điện thoại) | Phù hợp nhóm người dùng tiếng Việt. Là phán đoán, chưa có số liệu | Rà soát, phần lợi thế | Sau P3 |
| Lưu secret (playit, CurseForge) trong keyring của OS thay cho `secrets.toml` | Spec gốc muốn vậy; cần thêm thư viện và cách xử lý khi máy không có keyring | [Spec P4](specs/p4-truy-cap-internet.md#8-bảo-mật) | Sau P4 |

## Mod và nội dung

| Ý tưởng | Vì sao | Nguồn | Gợi ý |
| --- | --- | --- | --- |
| Hàng đợi thay đổi mod khi server đang chạy: bật/tắt, gỡ, cập nhật được ghi lại rồi áp dụng lúc server tắt | Hiện chỉ thêm được mod mới khi đang chạy; jar đang mở không thay được (nhất là trên Windows) | [Spec trình duyệt mod](specs/p3-trinh-duyet-mod.md), mục Không làm | Sau P3 |
| Thư viện ảnh, changelog từng bản, danh sách mod trong một modpack ở khung chi tiết | Giúp chọn mod và chọn bản pack dễ hơn | Như trên | Sau P3 |
| Trang Tệp: tìm theo nội dung, giải nén `.7z` / `.rar` (tải từ URL, `.tar`/`.tar.gz`, xem ảnh/NBT đã vào [spec nâng cấp trang Tệp](specs/p3-nang-cap-trang-tep.md)) | Người dùng hay nhận world, modpack qua nhiều định dạng | [Spec trang Tệp](specs/p3-quan-ly-tep.md), mục Không làm | Sau P3 |

## Phân phối và chạy nền

| Ý tưởng | Vì sao | Nguồn | Gợi ý |
| --- | --- | --- | --- |
| App desktop Tauri | Cửa sổ riêng thay cho tab trình duyệt. Chủ dự án bỏ khỏi P4 ngày 2026-11-18: khay và trình duyệt đã đủ | Spec gốc P4; [spec P4](specs/p4-truy-cap-internet.md#39-không-làm-tauri) | Chưa xếp giai đoạn |
| Windows service thật (chạy khi chưa ai đăng nhập, tự bật lại khi controller crash) | P4 chỉ có tự chạy khi đăng nhập; service cần app khay riêng và đổi chỗ thư mục dữ liệu | [Spec P4](specs/p4-truy-cap-internet.md#310-khởi-động-cùng-máy) | Chưa xếp giai đoạn |
| Gói `.deb` và trình cài MSI | Spec gốc có; P4 dùng zip, tar.gz và tự cập nhật | Spec gốc | Chưa xếp giai đoạn |

## Từ spec gốc (P5)

Nhiều tài khoản và phân quyền theo server; nhật ký thao tác; API token; 2FA; MCP để AI điều khiển
server; proxy Velocity; Prometheus metrics; telemetry có hỏi ý kiến; thêm ngôn ngữ; Hangar và SpigotMC
làm nguồn plugin bên cạnh Modrinth.

## Đã cân nhắc và không làm

| Ý tưởng | Lý do |
| --- | --- |
| Nhiều node, một bảng điều khiển cho nhiều máy | Ngoài phạm vi v2.0 (spec gốc). Không phải thị trường của MineHost |
| Host thương mại, hạn mức theo gói | Ngoài phạm vi v2.0 (spec gốc) |
| Cloudflare Tunnel, Tailscale, ngrok làm cách cho người chơi vào server | Người chơi phải cài thêm phần mềm, hoặc địa chỉ đổi mỗi lần chạy ([spec P4 mục 3.3](specs/p4-truy-cap-internet.md#33-chỉ-playitgg-làm-tunnel-trong-p4)). Tailscale vẫn là cách hướng dẫn để quản lý từ xa |
| Tự xây lớp chống DDoS tầng 7 | Khi mở ra Internet thì dùng tunnel che IP thay vì tự chống |
| Bedrock Dedicated Server | Ngoài phạm vi v2.0; Geyser là đường ngắn hơn |
