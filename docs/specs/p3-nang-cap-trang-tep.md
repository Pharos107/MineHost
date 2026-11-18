# Spec — Nâng cấp trang Tệp (đợt 2)

Trạng thái: duyệt theo yêu cầu chủ dự án 2026-11-18. Bổ sung [spec trang Tệp](p3-quan-ly-tep.md).

## Vấn đề

1. Khi bật "Tìm cả thư mục con", người dùng không thấy trạng thái đang bật sau khi đã gõ chữ, bảng nhấp nháy trong lúc chờ debounce/tìm trên server, chưa nhảy nhanh tới thư mục chứa kết quả tìm và đổi thư mục chưa tự xóa từ khóa cũ.
2. Hộp thoại Di chuyển chỉ hiện chuỗi `/{dir}` và nút Lên trên, khó nhảy nhanh về gốc hoặc thư mục cha.
3. Chưa kéo thả tải lên được cả thư mục giữ nguyên cấu trúc thư mục con.
4. File log / văn bản lớn hơn 1 MiB, file ảnh (`server-icon.png`), và file NBT (`.dat`, `.nbt`) chỉ tải xuống được, chưa xem trước trực tiếp trên web.
5. Thiếu thao tác nhân bản file/thư mục, tính dung lượng thư mục theo yêu cầu, tải file về server từ URL, giải nén `.tar` / `.tar.gz` / `.tgz`, và xem/khôi phục/dọn thùng rác `trash/`.

## Hành vi

### Phần A — Tinh chỉnh UX bảng file
- **Nút tìm thư mục con:** khi bật, nút cài đặt ở cuối ô tìm hiển thị trạng thái đang bật (`active`).
- **Tìm mượt không nhấp nháy:** khi bật tìm thư mục con, `DataTable` không lọc cục bộ trong lúc chờ debounce và hiện trạng thái đang tìm nhẹ nhàng (spinner ở ô tìm) mà không xóa bảng hiện tại.
- **Đổi thư mục & mở thư mục chứa:** đổi thư mục ở trang Tệp thì xóa ô tìm; với kết quả tìm nằm trong thư mục con (`a/b.yml`), menu hàng có mục "Mở thư mục chứa".
- **Hộp thoại Di chuyển:** dùng breadcrumb bấm nhảy tới từng bậc (kể cả gốc).

### Phần B — Tính năng mới
- **Tải lên cả thư mục:** `FileDrop` đọc đệ quy cây thư mục được kéo thả (`webkitGetAsEntry`) và nút "Tải thư mục"; route `POST /files/upload?path=&relative=true` tạo thư mục con an toàn qua `SafeRoot::join`, vẫn giữ nguyên hành vi chặn `/` khi `relative` tắt.
- **Nhân bản:** `POST /files/copy` `{ from, to }` sao chép file hoặc thư mục đệ quy (bỏ qua symlink, chặn `server.jar` khi server đang chạy, từ chối đích đã có bằng `FILE_EXISTS`, từ chối chép thư mục vào chính nó). Menu hàng có "Nhân bản".
- **Tính dung lượng thư mục:** `GET /files/size?path=` trả `{ bytes, files, dirs, truncated }` (bỏ qua symlink, giới hạn tối đa 200 000 mục). Menu hàng thư mục có "Tính dung lượng", hiện kết quả ngay ở cột Kích thước của hàng đó.
- **Thùng rác của server (`trash/<id>/files/`):**
  - `GET /files/trash` liệt kê các đợt đã xóa (`id`, `deleted_at`, `items`, `bytes`).
  - `POST /files/trash/restore` `{ batch }` khôi phục về `server/` (từ chối nếu trùng tên bằng `FILE_EXISTS`, kiểm `server.jar` khi chạy).
  - `DELETE /files/trash?batch=` xóa một đợt hoặc toàn bộ thùng rác của server.
  - UI có nút "Thùng rác" mở hộp thoại xem, khôi phục và dọn sạch (có xác nhận).
- **Tải file từ URL:** `POST /files/fetch` `{ dir, url, filename? }` chạy thành Job `fetch_file`, chỉ nhận `http://` hoặc `https://`, stream vào `.part` tối đa 512 MiB, báo tiến độ qua toast rồi đổi tên nguyên tử.
- **Giải nén `.tar`, `.tar.gz`, `.tgz`:** mở rộng `POST /files/extract` và nút "Giải nén" cho `.tar`, `.tar.gz`, `.tgz` với cùng quy tắc an toàn như zip (bỏ qua symlink/hardlink/thiết bị, kiểm `PATH_OUTSIDE_ROOT`, `FILE_EXISTS`, `NOT_ENOUGH_DISK`, giới hạn 100 000 mục).
- **Xem trước log/file lớn, ảnh và NBT ngay ở `/servers/:id/files/edit`:**
  - `GET /files/preview?path=`: đọc tối đa 128 KiB cuối của file văn bản lớn (cắt ở ranh giới UTF-8 hợp lệ, từ chối file nhị phân bằng `FILE_NOT_TEXT`), hiện ở `CodeEditor` chế độ chỉ đọc.
  - `GET /files/raw?path=`: trả nội dung ảnh (`png`, `jpg`, `jpeg`, `gif`, `webp`, kèm `X-Content-Type-Options: nosniff`; không phục vụ `svg`/`html`) để xem trước kèm kích thước ảnh.
  - `GET /files/nbt?path=`: giải nén gzip nếu cần và đọc NBT (`.dat`, `.dat_old`, `.nbt`, tối đa 16 MiB) thành JSON đọc được trong `CodeEditor` chỉ đọc.

## Tiêu chí chấp nhận

1. Bật tìm thư mục con: nút cài đặt nổi rõ, gõ chữ không chớp thông báo trống giả, đổi thư mục tự xóa ô tìm, mở được thư mục chứa từ kết quả tìm; `MoveDialog` bấm được từng bậc breadcrumb.
2. Kéo thả hoặc chọn thư mục tải lên giữ nguyên đường dẫn con; upload thường có `/` vẫn bị từ chối như cũ.
3. Nhân bản file/thư mục thành công và chặn trùng tên/chép vào chính nó; tính dung lượng thư mục hiện số byte trên bảng.
4. Thùng rác liệt kê, khôi phục được mục đã xóa (trừ khi trùng tên), và xóa sạch được sau khi xác nhận.
5. `fetch` tải file từ server HTTP cục bộ trong test thành công, chặn URL không phải http(s) và tên file trùng; giải nén `.tar.gz` an toàn và chặn mục `../evil`.
6. Xem trước được file văn bản > 1 MiB (128 KiB cuối), ảnh PNG, và file NBT gzip/không nén dạng JSON chỉ đọc; mọi test Rust + UI + `gen:api` xanh, không gọi mạng thật.

## Không làm

- Giải nén `.7z`, `.rar` (chưa có crate phù hợp trong workspace, giữ ở [backlog](../backlog.md)).
- Tìm theo nội dung bên trong mọi file, tự dọn `trash/` theo tuổi/dung lượng (giữ ở [backlog](../backlog.md)).
- Xem file `.mca` region thô.
