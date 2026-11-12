# Spec — Làm mới giao diện các trang chung (P2)

## Vấn đề

Trang chủ, Máy chủ và Cài đặt chỉ có ô số liệu và bảng. Người mở MineHost muốn biết ngay server nào
đang chạy, bạn bè vào bằng địa chỉ nào và ai đang chơi, nhưng những thứ đó nằm rải rác hoặc phải mở
từng server mới thấy. Bảng màu xanh đêm và font hệ thống làm giao diện trông như mọi bảng quản trị khác.

## Hành vi

- **Bảng màu "Đá và cỏ".** Nền đá ấm (không đen tuyền), nhấn xanh cỏ, đỏ redstone cho lỗi; có bản sáng
  và tối. Mặc định theo hệ điều hành; nút mặt trời / mặt trăng cạnh ô đổi ngôn ngữ chuyển sang bản còn
  lại và nhớ trên trình duyệt đó. Chữ là Be Vietnam Pro (vẽ riêng cho tiếng Việt), đóng gói cùng bản build.
- **Hoa văn khối.** Mỗi server có một lát cắt địa hình dạng khối sinh từ id: cùng server luôn cùng hình,
  mỗi server một "biome" màu riêng. Dùng ở đầu thẻ server, cạnh tên server trong bảng, và làm logo.
  Đây là điểm nhấn duy nhất; phần còn lại giữ yên.
- **Trang chủ.** Trên cùng là dải số liệu của máy (CPU, RAM, ổ đĩa, tác vụ đang chạy). Dưới là mỗi
  server một thẻ, server đang chạy lên trước. Thẻ có: tên, trạng thái, phiên bản và
  RAM, địa chỉ vào kèm nút chép, đầu skin những người đang online (tối đa 5, còn lại "+N"), nút mở
  console, bật / tắt / khởi động lại. Cuối trang là 5 tác vụ gần nhất.
- **Máy chủ.** Lọc theo trạng thái, tìm theo tên, xem dạng thẻ (mặc định) hoặc bảng. Lựa chọn được nhớ
  trên trình duyệt đó. Thông tin máy chuyển hẳn sang trang Cài đặt.
- **Cài đặt.** Mỗi mục một thẻ có biểu tượng; màn hình rộng thì tiêu đề và mô tả bên trái, điều khiển
  bên phải.

Component mới trong `ui/src/ui/`: `BlockBanner`, `ServerCard` + `ServerCardGrid`, `AvatarStack`,
`StatStrip`, `SegmentedControl`. `Card` thêm `icon` và `layout="split"`; cột của `DataTable` thêm `nowrap`.

## Tiêu chí chấp nhận

- Cùng id server luôn ra cùng hoa văn; id khác ra hình khác (Vitest).
- Trang chủ: server đang chạy lên trước, thẻ hiện địa chỉ và người đang online (Vitest).
- Máy chủ: đổi sang Bảng thì lần mở sau vẫn là Bảng (Vitest).
- Nút đổi giao diện chuyển sáng ↔ tối và nhớ lựa chọn; console đổi màu theo ngay (Vitest, xem tay).
- Không trang nào cuộn ngang ở bề ngang điện thoại 375 px.

## Không làm

- Trang trong một server (Tổng quan, Console, Tệp, Người chơi, Sao lưu) chỉ đổi theo token; bố cục giữ nguyên.
- Không cho người dùng tự chọn bảng màu khác ngoài sáng / tối, hay đổi font.
