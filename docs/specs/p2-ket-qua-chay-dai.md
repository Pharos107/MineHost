# Kết quả chạy dài của P2

Tiêu chí 1 của [spec P2](p2-nhieu-server-va-van-hanh.md#tiêu-chí-hoàn-thành): 10 server Paper chạy
song song liên tục trên một máy Linux. Bộ nhớ controller sau giờ thứ nhất không tăng quá 20%, không
server nào bị kill ngoài ý muốn. Kèm theo là lần thử LAN của tiêu chí 4.

**Kết quả: đạt**, với thời gian rút ngắn từ 72 giờ xuống 56 giờ 38 phút. Chủ dự án quyết định dừng
sớm để phát hành `v0.3.0` vì các số đo đã đứng yên từ ngày đầu.

## Cách chạy

- Máy: Ubuntu 26.04, 12 nhân, 30 GB RAM. Máy dùng chung với vài dịch vụ khác đang chạy thật.
- Binary: bản release build từ commit `96c34c0` (cuối P2, sau #63). Bản `v0.3.0` được tag từ đúng
  code này, chỉ thêm commit đổi số phiên bản và tài liệu.
- Script: [`scripts/chay-dai.sh`](../../scripts/chay-dai.sh). Script dựng 10 server Paper 1.21.4,
  heap 512 MB, cổng 25600–25609, rồi ghi một dòng CSV mỗi 10 phút. Cả nhóm (controller và 10 Java)
  chạy trong một cgroup `MemoryMax=10G`, `CPUWeight=20`, `Nice=10` để không ảnh hưởng dịch vụ khác.
- Không có người chơi vào server, nên đây là tải nhàn rỗi. Trong lúc chạy, các server vẫn sinh
  world, lưu tự động, chạy tick bình thường.
- Mốc thời gian:
  - Bắt đầu: 2026-11-12 21:27:10 UTC, lúc server thứ 10 bật xong.
  - Dừng: 2026-11-15 06:05:56 UTC, bằng nút Thoát (`POST /app/shutdown`).
  - Một lần khởi động trước đó lúc 21:13 chỉ chạy 7 phút rồi dừng, không tính vào kết quả.

## Số đo

| Thời điểm (UTC) | RSS controller | Server đang chạy | RAM cả nhóm | Ổ trống |
| --- | --- | --- | --- | --- |
| 11-12 21:27 (bắt đầu) | 22 796 KiB | 10 | 10,74 GB | 32 GB |
| 11-12 22:27 (giờ thứ nhất) | 23 060 KiB | 10 | 10,74 GB | 32 GB |
| 11-13 03:17 | 10 880 KiB | 10 | 9,98 GB | 26 GB |
| 11-13 15:17 | 10 900 KiB | 10 | 10,11 GB | 25 GB |
| 11-14 03:17 | 11 084 KiB | 10 | 10,17 GB | 25 GB |
| 11-14 15:17 | 11 084 KiB | 10 | 10,19 GB | 25 GB |
| 11-15 03:17 | 11 088 KiB | 10 | 10,21 GB | 25 GB |
| 11-15 05:57 (mẫu cuối) | 11 088 KiB | 10 | 10,22 GB | 25 GB |

Có 341 mẫu. Mọi mẫu đều đủ 10 server đang chạy và cùng một bộ 10 PID Java. Nghĩa là không server
nào crash, bị kill hay bị khởi động lại trong cả lần chạy.

**Bộ nhớ controller.** RSS cao nhất là 23 088 KiB, cao hơn giờ thứ nhất 0,12%. Từ khoảng giờ thứ 3,
RSS giảm xuống còn khoảng 11 MB. Lý do là cgroup chạm trần 10 GB nên kernel đẩy một phần trang nhớ
của controller ra swap, chứ không phải controller nhả bộ nhớ. Đo lúc dừng: `VmRSS` 11 088 KiB cộng
`VmSwap` 7 244 KiB, tổng 18 332 KiB. Con số này vẫn thấp hơn mức của giờ thứ nhất, nên không có rò rỉ.

**Log.** Log controller của lần chạy này không có dòng `WARN` hay `ERROR` nào. Trạng thái server chỉ
đổi lúc bật và lúc bấm Thoát. Cgroup này không có lần OOM nào; swap dùng cao nhất 299 MB.

**Thoát.** Bấm Thoát thì cả 10 server chuyển `Stopping` → `Stopped` trong 4,4–5,0 giây, rồi
controller tự thoát. Tổng CPU của cả nhóm là 6 giờ 1 phút trên 56,7 giờ chạy.

## Thử LAN (tiêu chí 4)

Thử ngay sau lần chạy dài, dùng cùng binary và thư mục dữ liệu đó. Đặt mật khẩu qua API, rồi bật LAN.
Controller mở thêm listener ở địa chỉ LAN của máy (`192.168.1.22:7979`).

| Việc | Kết quả |
| --- | --- |
| Gọi `/servers` từ `127.0.0.1` khi chưa đăng nhập | `AUTH_REQUIRED` |
| Gọi `/servers` qua địa chỉ LAN khi chưa đăng nhập | `AUTH_REQUIRED` |
| Đăng nhập qua địa chỉ LAN | Nhận được cookie phiên và CSRF |
| Bật rồi tắt server "Chạy dài 1" qua địa chỉ LAN | `204`, trạng thái `running` rồi `stopped` |
| Gửi lệnh bật thiếu `X-CSRF-Token` | `FORBIDDEN`, `reason: csrf` |
| Gửi header `Host: evil.example` | `403` |

Request được gửi từ một container Docker (`172.17.0.3`) trên cùng máy, không qua loopback. Lần thử từ
một máy Windows khác trong LAN không làm được: tường lửa của máy chủ chặn mọi cổng TCP từ LAN, kể cả
SSH, còn ping thì vẫn qua. Lần thử này không sửa tường lửa. Muốn thử lại bằng máy thật thì mở cổng
7878 hoặc 7979 cho mạng LAN trước.

## Việc nên làm khác ở lần sau

- Ghi cả `VmSwap`, hoặc dùng PSS, bên cạnh `VmRSS`. Khi cgroup chạm trần, chỉ nhìn RSS sẽ tưởng
  controller nhả bộ nhớ.
- Đặt trần cgroup cao hơn tổng RSS dự kiến khoảng 20%, để swap không làm nhiễu số đo.
- Cho một người chơi giả (bot) vào vài server, để có tải thật thay vì server nhàn rỗi.
