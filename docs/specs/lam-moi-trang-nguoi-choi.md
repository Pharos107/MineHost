# Spec — Làm mới trang Người chơi

## Vấn đề

Trang Người chơi của P2 (spec P2, mục 6.7) là ba thẻ chồng nhau: ô thêm theo tên chiếm cả đầu trang dù
ít khi dùng, bảng Đang online trống vẫn chiếm một thẻ, bảng Đã từng vào chỉ có icon nhỏ để mở chi tiết.
Hộp thoại chi tiết xếp nút vào cột nhãn của `Facts`, nút đỏ nhạt khó đọc, và không cho biết nhanh
server đang mở cho ai (bao nhiêu op, whitelist bật hay tắt, bao nhiêu người bị cấm).

## Hành vi

- **Ô số** trên cùng: Đang chơi (số / tối đa), Whitelist (số người, dòng phụ "Đang bật" hoặc
  "Đang tắt"), Op, Bị cấm.
- **Danh sách gộp** bên trái: ô tìm theo tên, nút "Thêm" (mở hộp thoại thêm theo tên như cũ, giữ mở
  để thêm liền nhiều người), dải lọc Tất cả / Op / Whitelist / Bị cấm (nhóm Đang chơi đã đứng đầu nên không có bộ lọc riêng). Hai nhóm: Đang chơi
  (luôn hiện khi lọc Tất cả, trống thì ghi "Chưa có ai đang chơi.") và Không online. Mỗi hàng: đầu skin
  có chấm xanh khi đang chơi, tên, dòng phụ ("Đang chơi", "Vào 2 giờ trước", "Chưa vào"), huy hiệu op / bị cấm / đóng băng (whitelist
  chỉ hiện ở khung chi tiết, để tên dài không bị cắt).
- **Khung chi tiết** bên phải, thay hộp thoại: skin 3D (hoặc đầu skin lớn), tên, huy hiệu, UUID có nút
  chép, lý do cấm. Bốn nhóm, mỗi nhóm có tiêu đề kèm icon:
  - Quyền: hai công tắc Whitelist và Op (làm được cả khi server tắt).
  - Tương tác: Đóng băng / Bỏ đóng băng, Về điểm spawn, chọn người và Dịch chuyển.
  - Nhắn tin: ô nội dung, nút Nhắn tin và Hiện giữa màn hình.
  - Kỷ luật (tiêu đề đỏ, để cuối): ô lý do, Đuổi, Cấm / Bỏ cấm.
- Chưa chọn ai thì khung phải ghi "Chọn một người chơi". Màn hình hẹp (dưới 1024 px) chỉ hiện một
  bên: chọn người thì chi tiết thay danh sách, có nút quay lại.
- Thao tác nào bị mờ và vì sao vẫn theo bảng ở spec P2, mục 6.7.

Component mới: `SelectList`, `SelectListGroup`, `SelectListItem`, `DetailHeader`, `DetailSection`.
`Avatar` thêm cỡ `xl` và `online` (chấm xanh). `SegmentedControl` thêm `size="sm"`. Sửa `SplitView`: màn
rộng chưa chọn gì thì khung chi tiết vẫn hiện (trước bị luật của màn hẹp ẩn mất).

## Tiêu chí chấp nhận

- Ô số đếm đúng; lọc theo nhóm và tìm theo tên ra đúng hàng (Vitest).
- Bấm một người thì hàng đó được chọn và khung chi tiết hiện người đó (Vitest).
- Công tắc Whitelist / Op gửi `whitelist_add`/`whitelist_remove`, `op`/`deop` theo trạng thái (Vitest).
- Các thử nghiệm cũ của hộp thoại (nút mờ khi tắt, cấm kèm lý do, dịch chuyển, nhắn tin, bỏ cấm người
  chỉ có trong danh sách cấm) vẫn qua với khung chi tiết (Vitest), E2E người vào hiện trong nhóm Đang chơi.
- Xem tay ở chế độ sáng, tối và bề ngang 375 px: không cuộn ngang.

## Không làm

- Thời gian đã chơi, ping, số lần vào như mc-controller: backend chưa ghi, cần RCON hoặc plugin.
- Thao tác hàng loạt (chọn nhiều người).
