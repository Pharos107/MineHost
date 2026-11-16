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
- **Thanh bên:** cạnh tên server có nút icon Bật, Khởi động lại, Tắt theo trạng thái, dưới tên là trạng thái;
  dùng chung logic với nút ở đầu trang (hỏi EULA trước khi bật lần đầu). "Buộc dừng" chỉ ở menu "…"
  đầu trang. Server đang cài hoặc hỏng thì chỉ có trạng thái. Mục "Tất cả máy chủ"
  ghim ở đáy thanh bên, ngay trên phần đổi ngôn ngữ và Thoát.
- **Ô số:** CPU, RAM (kèm thanh mức so với RAM tối đa, từ 90 % thì đổi màu cảnh báo), Người chơi
  (chỉ số đang chơi / tối đa, không liệt kê tên), Thời gian chạy, Dung lượng. Lưới giãn hết chiều ngang.
- **Biểu đồ:** CPU, RAM và số người chơi 1 giờ qua. Nút "Cạnh nhau" / "Từng hàng" đổi cách xếp, nhớ trên
  trình duyệt đó; cạnh nhau là ba cột trên màn hình rất rộng, hai cột khi vừa; màn hình hẹp luôn một cột. Nút "Xem chi tiết" trên mỗi biểu đồ mở hộp thoại có biểu
  đồ lớn hơn và bốn số của cả giờ: hiện tại, trung bình, cao nhất, thấp nhất.
- **Dưới biểu đồ** (cả khi server tắt), hai thẻ cạnh nhau:
  - **Thông tin server:** Java, RAM tối thiểu – tối đa, cổng, tự bật cùng MineHost, tự bật lại khi crash,
    giờ khởi động lại hằng ngày, lịch sao lưu tự động.
  - **Console gần đây:** khung log thấp chỉ đọc (giữ 200 dòng), nút "Mở console" sang trang Console.

Component: `StatTile` thêm `meter`, `NavSection` thêm `actions` và `extra`, `PageHeader` thêm `eyebrow`,
`AppShell` thêm `navEnd`, `Terminal` thêm `size="sm"`, thêm `Columns` (1, 2 hoặc 3 cột).

## Tiêu chí chấp nhận

- Dòng phiên bản ghép đúng loader, bản loader, Minecraft, modpack (Vitest).
- Nút ở thanh bên gửi đúng lệnh, bật lần đầu vẫn hỏi EULA, không có menu Buộc dừng (Vitest).
- Ô Người chơi chỉ hiện số; ô RAM có thanh mức đúng phần trăm (Vitest).
- Thẻ Thông tin server hiện đúng từng cài đặt; lịch sao lưu đọc thành câu; dòng console mới hiện ở thẻ
  Console gần đây; có biểu đồ Người chơi (Vitest).
- Đổi sang "Từng hàng" thì lần mở sau vẫn là từng hàng; hộp chi tiết có trung bình, cao nhất, thấp nhất (Vitest).
- Xem tay ở chế độ sáng, tối và bề ngang 375 px: không cuộn ngang.

## Không làm

- Chọn khoảng thời gian dài hơn 1 giờ: backend chỉ giữ 1 giờ mẫu.
- TPS, MSPT, chunk, entity, lưu lượng mạng như mc-controller: cần RCON hoặc plugin, MineHost điều khiển
  qua console.
- Chế độ bảo trì.
