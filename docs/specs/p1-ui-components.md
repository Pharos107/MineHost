# Spec — Bộ component giao diện dùng chung

Thuộc P1, làm trước PR-13. Kế hoạch: [p1-ke-hoach-pr.md](p1-ke-hoach-pr.md) (PR-13a, 13b, 13c).

## Vấn đề

Ở dự án cũ (mc-controller), mỗi trang tự viết lại cùng một thứ với class riêng: 6 kiểu thông báo
(`admin-notice`, `server-admin-notice`, `settings-callout`, `create-wizard-notice`, `console-notice`,
`maintenance-notice`), 5 kiểu modal, 6 kiểu đầu trang (`overview-header`, `console-header`,
`startup-hero`…) và 4 kiểu card. File CSS phình tới hơn 10.000 dòng, sửa một chỗ không ăn sang chỗ
khác, giao diện mỗi trang một kiểu.

## Hành vi

Trang chỉ được ghép từ component trong `ui/src/ui/`. Trang không tự đặt màu, cỡ chữ, bo góc hay
khoảng cách; mọi giá trị đó là token trong `ui/src/ui/tokens.css`.

### Token

- Màu theo vai trò, không theo tên màu: `--mh-bg`, `--mh-surface`, `--mh-surface-raised`, `--mh-border`,
  `--mh-text`, `--mh-text-muted`, `--mh-primary`, `--mh-on-primary`, `--mh-success`, `--mh-warning`,
  `--mh-danger`, `--mh-info` và bản nền nhạt `--mh-*-soft` cho từng trạng thái.
- Theo chế độ sáng/tối của hệ điều hành. Từ P2 dùng bảng màu "Đá và cỏ" (nền đá ấm, nhấn xanh cỏ;
  xem [p2-lam-moi-giao-dien.md](p2-lam-moi-giao-dien.md)); bản sáng là bản tương ứng đủ tương phản.
- Khoảng cách `--mh-space-1..8` (bội số 4px), bo góc `--mh-radius-sm|md|lg`, chữ `--mh-font-sans|mono`.
  Chữ chính là Be Vietnam Pro, đóng gói cùng bản build (MineHost chạy được khi không có mạng).

### Component

| Nhóm | Component | Dùng ở |
| --- | --- | --- |
| Cơ bản (13a) | `Button` (primary, secondary, danger, ghost; cỡ sm, md; `loading`), `IconButton` (bắt buộc `label`) | mọi trang |
| | `Card` (tiêu đề, mô tả, nút ở góc, thân) | chi tiết, cài đặt |
| | `PageHeader` (tiêu đề, dòng phụ, vùng nút) | mọi trang |
| | `Notice` (info, success, warning, danger; tiêu đề, nội dung, nút) | lỗi, cảnh báo RAM, "có hiệu lực sau khi khởi động lại" |
| | `Badge`, `StatusBadge` (ánh xạ 7 trạng thái server ra màu và nhãn đã dịch) | danh sách, chi tiết |
| | `Spinner`, `EmptyState` | tải dữ liệu, danh sách trống |
| | `Stack` (xếp hàng/cột, khoảng cách theo token), `Facts` (danh sách nhãn: giá trị) | mọi trang, thông tin máy và server |
| | `AppShell` (thanh trên: tên app, đổi ngôn ngữ, vùng nút; thân trang) | mọi trang |
| Form (13b) | `Field` (nhãn, gợi ý, lỗi; tự nối `id` và `aria-describedby`) | wizard, cài đặt |
| | `TextInput`, `Select`, `Switch`, `Slider` (có vạch "đề xuất") | wizard, cài đặt |
| | `Steps` (wizard 3 bước), `ProgressBar` (có số %, hoặc chạy vô định) | wizard, tiến độ Job |
| | `ChoiceGroup` (chọn một trong vài thẻ lớn, radio bên dưới; thêm ở PR-13) | wizard: loại server |
| Lớp phủ (13c) | `Dialog` (dựa trên `<dialog>`: khóa focus, Esc để đóng) | EULA |
| | `ConfirmDialog` (tùy chọn bắt gõ lại một chuỗi mới cho bấm) | Buộc dừng, Xóa, Thoát |
| | `Tabs` (phím mũi tên chuyển tab) | chi tiết server |
| | `Menu` (nút "…", Esc và bấm ra ngoài để đóng) | Buộc dừng |
| | `CopyField` (giá trị kèm nút chép vào clipboard, báo "Đã chép") | địa chỉ LAN |
| Bảng (13d) | `DataTable` (cột khai báo, sắp xếp, tìm không dấu, thao tác theo hàng, dạng thẻ khi hẹp), `SearchInput` | danh sách server; P2: file, backup, người chơi |
| Nội dung (P3) | `ProjectCard` (ảnh, tên, mô tả một dòng, số lượt tải, nhãn loader và phiên bản) | tìm mod, tìm modpack |

### Bảng

Dự án cũ có 17 bảng với hơn 10 class riêng (`pl-table`, `files-table`, `backups-table`…) và độ rộng
cột viết tay bằng `style`, dù khung giống hệt nhau. `DataTable` gom khung đó lại; trang chỉ khai báo:

- `columns`: tiêu đề, cách hiện ô, `sortValue` nếu sắp xếp được, `align`, `width` (`xs|sm|md|lg` theo token).
- `actions`: nhãn, icon, `tone: "danger"`, `hidden`/`disabled` theo từng hàng. `inlineActions` (mặc định 2)
  thao tác đầu hiện thành icon, phần còn lại vào menu "…". Nhãn nút kèm tên hàng ("Xóa: Survival").
- `searchText`: có thì hiện ô tìm kiếm; tìm không phân biệt dấu ("duc" ra "Đức").
- `toolbar` (nút bên trái thanh trên), `empty` (hiện thay bảng khi chưa có hàng), `loading`.

Chân bảng ghi "Hiển thị x/y". Khung hẹp dưới 560px (theo container, không theo màn hình) thì mỗi hàng
thành một thẻ, nhãn cột hiện trước từng giá trị. `Menu` đặt `position: fixed` để không bị vùng cuộn
của bảng cắt, và tự mở lên trên khi gần đáy màn hình.

Chưa làm, thêm khi P2 có trang file và người dùng: chọn nhiều hàng (checkbox) cùng thao tác hàng loạt,
phân trang và chọn số dòng mỗi trang, sắp xếp và tìm phía server.

Icon dùng `lucide-react` qua component, không chép SVG vào trang.

### Quy tắc

- CSS của component dùng CSS Modules (`*.module.css`) nên class không rò sang nơi khác. Trang không
  ghi đè style của component; cần kiểu mới thì thêm `variant` vào component.
- Component không gọi API và không biết về server; `StatusBadge` chỉ nhận trạng thái.
- Chuỗi hiển thị bên trong component (ví dụ "Đóng", "Đã chép", nhãn trạng thái) đi qua `t(...)`.
- Mọi thứ bấm được dùng được bằng bàn phím và có viền focus rõ.
- Trang `#/ui` (chỉ có khi `npm run dev`) bày mọi component và biến thể để xem và so sánh.
  Trang này không có trong bản build, nên chuỗi mẫu trong đó được ghi thẳng, không qua i18n.
- Ô cần gõ ngay khi hộp thoại mở thì gắn `data-autofocus`, vì `showModal()` tự focus nút đầu tiên.

## Tiêu chí chấp nhận

- `npm run lint`, `typecheck`, `build` và `npm test` (Vitest) đều qua.
- Test hành vi: `Button` khi `loading` thì không bấm được; `IconButton` có `aria-label`; `Field` nối
  đúng `aria-describedby` khi có lỗi; `StatusBadge` có nhãn cho đủ 7 trạng thái ở cả `vi` và `en`;
  `ConfirmDialog` chỉ cho bấm khi gõ đúng chuỗi; `Tabs` chuyển bằng phím mũi tên; `Menu` đóng khi bấm
  Esc và tự mở lên trên khi gần đáy; `CopyField` gọi clipboard; `DataTable` sắp tăng/giảm và đổi `aria-sort`, tìm không dấu, ẩn
  thao tác theo hàng, đưa thao tác dư vào menu, hiện `empty` khi không có hàng; `vi.json` và `en.json` có đủ khóa như nhau.
- Xem trang `#/ui` ở cả chế độ sáng và tối, rộng 375px và 1280px: không vỡ bố cục.

## Không làm

Toast, chọn file, biểu đồ, nền "cosmic" có hiệu ứng của dự án cũ. Router và
các trang thật thuộc PR-13 và PR-14. Thêm component mới khi có trang thật cần tới nó.
