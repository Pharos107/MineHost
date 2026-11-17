# Spec — Trang Tệp nâng cấp và trình sửa code

Trạng thái: đã duyệt 2026-11-17 (chủ dự án chọn: giữ bố cục bảng + trang sửa riêng, CodeMirror 6,
thêm chọn nhiều, di chuyển, tìm trong thư mục con, nén / giải nén zip). File này bổ sung
[spec P2](p2-nhieu-server-va-van-hanh.md) mục 5.8 và 6.6; chỗ nào hai bên khác nhau thì file này đúng.
Làm trước PR-38 (`v0.5.0`).

## Vấn đề

- Trình sửa hiện tại là một ô văn bản trơn: không số dòng, không tô màu, không tìm và thay. Sửa
  `server.properties`, `config.yml` của plugin hay `*.toml` của mod dễ sai thụt lề mà không thấy.
- Bảng file chỉ làm từng mục một: muốn xóa hay tải 20 file thì bấm 20 lần, không tải được cả thư mục.
- Không chuyển được file sang thư mục khác (phải tải xuống rồi tải lên lại).
- Ô tìm của bảng chỉ lọc thư mục đang mở; không biết `config.yml` của plugin nào nằm ở đâu.
- Người dùng hay nhận world, bộ plugin dạng `.zip` nhưng MineHost không giải nén được, phải giải nén
  trên máy rồi tải từng file lên.

Dự án cũ (mc-controller) đã có trình sửa Monaco với thanh trạng thái, chọn nhiều, di chuyển, tìm
trong thư mục con. Spec này lấy các tính năng đó, đổi Monaco sang CodeMirror 6 (nhẹ hơn nhiều lần,
nhúng được vào binary, dùng được trên điện thoại).

## Hành vi

### 1. Trình sửa code `/servers/:id/files/edit`

- Component `CodeEditor` trong `ui/src/ui/` bọc CodeMirror 6. Mã CodeMirror chỉ tải khi mở trang sửa
  (`import()` động); tải lỗi thì quay về ô văn bản cũ, vẫn lưu được.
- Có: số dòng, tô dòng đang đứng, khớp ngoặc, gập khối, hoàn tác / làm lại, tìm và thay (Ctrl+F,
  hộp tìm của CodeMirror dịch theo i18n), Tab thụt lề 2 dấu cách, tùy chọn xuống dòng.
- Màu theo token của MineHost (`tokens.css`), tự đổi theo giao diện sáng / tối. Font mono của hệ thống.
- Tô cú pháp theo đuôi file:

| Đuôi | Ngôn ngữ |
| --- | --- |
| `json`, `json5`, `mcmeta` | JSON |
| `yml`, `yaml` | YAML |
| `toml` | TOML |
| `properties`, `cfg`, `conf`, `ini` | Properties |
| `sh`, `bash` | Shell |
| khác (`txt`, `log`, `mcfunction`, `snbt`…) | Văn bản thường |

- Đầu trang: nút Quay lại và breadcrumb như cũ; bên phải nút **Lưu** (Ctrl+S), **Tải xuống**, công tắc
  **Xuống dòng** (nhớ trong `localStorage`).
- Dưới trình sửa là thanh trạng thái: "Dòng 12, Cột 5", ngôn ngữ, kích thước file, trạng thái
  ("Chưa lưu" / "Đã lưu").
- Giữ nguyên: cảnh báo khi rời trang còn thay đổi chưa lưu, xử lý `FILE_CHANGED` ("Tải lại bản mới" /
  "Chép thay đổi của tôi"), file lớn hơn 1 MiB hoặc không phải văn bản thì chỉ tải xuống.

### 2. Bảng file `/servers/:id/files`

- **Thanh công cụ một hàng** ở đầu bảng (prop `toolbar` của `DataTable`): Tải lên, Tệp mới, Thư mục
  mới; ô tìm của bảng ở bên phải.
- **Bỏ ô kéo thả riêng.** Kéo file thả vào bảng là tải lên đúng thư mục bảng đang mở (lớp phủ "Thả để
  tải lên <thư mục>"), kể cả khi thư mục trống.
- **Thông báo nổi** (component `Toast`, góc phải dưới, không đẩy bảng xuống): xong việc thì hiện 5 giây
  rồi tự tắt, có nút đóng. Việc còn chạy (tải lên, nén, giải nén) giữ toast kèm thanh tiến độ tới khi
  xong, rồi đổi thành kết quả hoặc lỗi. Lỗi trong hộp thoại vẫn hiện trong hộp thoại.
- **Chọn nhiều:** `DataTable` có thêm prop `selection` (ô chọn đầu mỗi hàng, ô chọn tất cả ở tiêu đề,
  chỉ tính các hàng đang hiện sau khi lọc). Khi có mục được chọn, thanh công cụ đổi thành thanh hàng
  loạt: "Đã chọn N", **Tải xuống** (zip), **Di chuyển**, **Nén**, **Xóa**, **Bỏ chọn**. Đổi thư mục thì
  bỏ chọn.
- **Menu mỗi hàng:** Tải xuống (thư mục thì tải thành zip), Đổi tên, Di chuyển, Nén thành zip, Giải nén
  (chỉ file `.zip`), Xóa.
- **Di chuyển:** hộp thoại chọn thư mục đích: breadcrumb, danh sách thư mục con (bấm để vào), nút
  "Chuyển vào đây". Không chọn được chính thư mục đang chuyển hay thư mục con của nó. Nhiều mục thì
  chuyển lần lượt, dừng ở mục lỗi đầu tiên và báo mục đó.
- **Xóa nhiều:** một hộp thoại xác nhận "Chuyển N mục vào thùng rác", xóa lần lượt như xóa một mục.
- **Tìm trong thư mục con:** nút cài đặt nhỏ ở cuối ô tìm của bảng mở menu có mục bật/tắt "Tìm cả thư
  mục con", **mặc định tắt** (ô tìm chỉ lọc thư mục đang mở). Bật thì ô đổi chữ gợi ý thành "Tìm trong
  mọi thư mục con"; gõ xong 300 ms thì gọi route tìm (mục 3) từ thư mục đang mở và bảng hiện kết quả,
  tên là đường dẫn tính từ thư mục đó (ví dụ `Essentials/config.yml`). Mọi thao tác hàng và chọn nhiều
  vẫn dùng được trên kết quả; đổi tên thì đổi ngay trong thư mục chứa mục đó. Quá giới hạn thì ghi
  "Chỉ hiện 200 kết quả đầu" dưới bảng.
- **Nén:** hộp thoại hỏi tên file zip (mặc định `<tên mục>.zip`, chọn nhiều thì `luu-tru.zip`), tạo
  trong thư mục đang mở. Chạy thành Job, tiến độ hiện như các Job khác.
- **Giải nén:** hộp thoại hai lựa chọn: "Vào thư mục mới `<tên zip>`" (mặc định) hoặc "Vào thư mục
  hiện tại". Chạy thành Job.

### 3. Route mới (`/api/v1/servers/{id}/files/...`)

| Route | Việc |
| --- | --- |
| `GET /search?path=&q=` | Tìm theo tên (chứa chuỗi, không phân biệt hoa thường) trong `path` và mọi thư mục con. Trả `{ items: [{ path, kind, size?, modified? }], truncated }`, tối đa 200 mục, duyệt tối đa 100 000 mục. Không đi theo symlink. `q` rỗng là `INVALID_INPUT`. |
| `GET /archive?paths=<JSON>` | Stream một file zip gồm các đường dẫn trong mảng JSON (file hoặc thư mục, cùng thư mục cha). Tên tải về: tên mục nếu chỉ một, không thì `<tên thư mục>.zip` (gốc là `server.zip`). Không ghi file tạm. |
| `POST /compress` `{ paths, dest }` | Job `compress_files`: nén `paths` thành file zip `dest`. `dest` đã có là `FILE_EXISTS` (trả ngay, không tạo Job). Lỗi giữa chừng thì xóa file dở. |
| `POST /extract` `{ path, dest }` | Job `extract_files`: giải nén `path` vào thư mục `dest` (tạo nếu chưa có). |

- Route `rename` (di chuyển dùng chính route này) thêm kiểm tra: chuyển thư mục vào chính nó hoặc
  thư mục con của nó là `INVALID_INPUT`.
- Mọi đường dẫn đi qua `SafeRoot::join`. `server.jar` ở gốc khi server đang chạy vẫn bị chặn như cũ.
- Server đang chạy vẫn nén, giải nén, tìm được (giống các thao tác tệp khác).

**An toàn khi giải nén** (kiểm hết trước khi ghi file đầu tiên):

- Tên mục không nằm gọn trong thư mục đích (`..`, đường dẫn tuyệt đối) → `PATH_OUTSIDE_ROOT`, không
  giải nén gì. Mục symlink bị bỏ qua.
- Mục nào trùng file hoặc thư mục đã có → `FILE_EXISTS` kèm đường dẫn đó, không ghi đè.
- Tổng dung lượng khai báo vượt dung lượng trống của đĩa trừ 1 GiB → `NOT_ENOUGH_DISK`. Khi ghi, mỗi mục bị cắt ở dung lượng đã khai báo (chống zip bomb khai báo sai).
- Quá 100 000 mục → `INVALID_INPUT`.
- Lỗi giữa chừng: các file đã ghi được giữ lại, Job báo lỗi kèm đường dẫn mục lỗi.

## Tiêu chí chấp nhận

1. Mở `server.properties`, `config.yml`, `*.toml`, `*.json` thấy số dòng và tô màu đúng ngôn ngữ;
   Ctrl+F tìm và thay được; Ctrl+S lưu; giao diện tối đổi màu trình sửa.
2. Mã CodeMirror nằm trong chunk riêng, không làm tăng chunk chính quá 5 KB (gzip).
3. Chọn 3 file, bấm Xóa: cả 3 vào `trash/`. Chọn 2 mục, Tải xuống: nhận một zip chứa đúng 2 mục.
4. Di chuyển `a.txt` vào `plugins/`: file nằm ở `plugins/a.txt`. Chuyển `plugins` vào `plugins/x`
   bị từ chối.
5. Tìm `config` từ gốc thấy `plugins/Essentials/config.yml`; bấm vào thì mở trình sửa.
6. Nén thư mục `world` thành `world.zip`, xóa `world`, giải nén `world.zip` vào thư mục hiện tại: server
   bật lên vào được world cũ.
7. Zip có mục `../evil.txt` bị từ chối, không ghi file nào. Zip trùng tên file đã có bị từ chối.
8. Test Rust cho tìm, zip tải xuống, nén, giải nén (kể cả zip độc), test UI cho chọn nhiều, di chuyển,
   trình sửa; không gọi mạng thật.

## Không làm

- Bố cục cây thư mục + trình sửa song song, mở nhiều tab file cùng lúc.
- Tìm theo nội dung file, sửa file lớn hơn 1 MiB, xem ảnh / NBT.
- Tải về từ URL (dự án cũ có): để sau, ghi backlog.
- Giải nén `.tar.gz`, `.7z`, `.rar`; nén dạng khác zip.
- Phân quyền theo file, chmod.
- Kiểm tra cú pháp YAML / JSON khi lưu (chỉ tô màu).

## Chia PR

| PR | Việc |
| --- | --- |
| 47 | Spec này; component `CodeEditor` (CodeMirror 6) và trang sửa mới |
| 48 | Backend: tìm, zip tải xuống, nén, giải nén, chặn di chuyển vào chính nó |
| 49 | Bảng file: chọn nhiều, thao tác hàng loạt, di chuyển, tìm, nén / giải nén |
