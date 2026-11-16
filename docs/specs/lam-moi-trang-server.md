# Spec — Làm mới trang trong một server

## Vấn đề

Trang Tổng quan của một server chỉ có tên, trạng thái, bốn ô số và hai biểu đồ cao xếp chồng, nên
trống nhiều mà vẫn thiếu thông tin người dùng hay hỏi: server chạy loader gì, bản nào, modpack nào,
bao nhiêu người đang chơi. Muốn bật / tắt server thì phải quay về Tổng quan hoặc trang Máy chủ.
Dự án cũ (mc-controller) đã có những thứ này: dòng runtime và phiên bản trên tên, số người chơi cạnh
CPU / RAM, nút bật / tắt ở thanh bên.

## Hành vi

- **Đầu trang** (mọi tab của server): dòng nhỏ phía trên tên, ví dụ
  "Fabric 0.16.14 · Minecraft 1.20.1 · Prominence II v4.1.8". Thiếu phần nào thì bỏ phần đó.
- **Thanh bên:** dưới tên server có trạng thái và nút icon Bật, Khởi động lại, Tắt theo trạng thái,
  dùng chung logic với nút ở đầu trang (hỏi EULA trước khi bật lần đầu). "Buộc dừng" chỉ ở menu "…"
  đầu trang. Server đang cài hoặc hỏng thì chỉ có trạng thái.
- **Ô số:** CPU, RAM (kèm thanh mức so với RAM tối đa, từ 90 % thì đổi màu cảnh báo), Người chơi
  (chỉ số đang chơi / tối đa, không liệt kê tên), Thời gian chạy, Dung lượng. Lưới giãn hết chiều ngang.
- **Biểu đồ:** vẫn hai biểu đồ CPU và RAM 1 giờ qua. Nút "Cạnh nhau" / "Hai hàng" đổi cách xếp, nhớ trên
  trình duyệt đó; màn hình hẹp luôn một cột. Nút "Xem chi tiết" trên mỗi biểu đồ mở hộp thoại có biểu
  đồ lớn hơn và bốn số của cả giờ: hiện tại, trung bình, cao nhất, thấp nhất.

Component: `StatTile` thêm `meter`, `NavSection` thêm `actions` và `extra`, `PageHeader` thêm `eyebrow`, thêm `Columns`.

## Tiêu chí chấp nhận

- Dòng phiên bản ghép đúng loader, bản loader, Minecraft, modpack (Vitest).
- Nút ở thanh bên gửi đúng lệnh, bật lần đầu vẫn hỏi EULA, không có menu Buộc dừng (Vitest).
- Ô Người chơi chỉ hiện số; ô RAM có thanh mức đúng phần trăm (Vitest).
- Đổi sang "Hai hàng" thì lần mở sau vẫn là hai hàng; hộp chi tiết có trung bình, cao nhất, thấp nhất (Vitest).
- Xem tay ở chế độ sáng, tối và bề ngang 375 px: không cuộn ngang.

## Không làm

- Chọn khoảng thời gian dài hơn 1 giờ: backend chỉ giữ 1 giờ mẫu.
- TPS, MSPT, chunk, entity, lưu lượng mạng như mc-controller: cần RCON hoặc plugin, MineHost điều khiển
  qua console.
- Chế độ bảo trì.
