# Spec — Trình duyệt mod và làm gọn trang Mod

Trạng thái: nháp 2026-11-16, chờ chủ dự án duyệt. File này bổ sung [spec P3](p3-mod-va-modpack.md)
mục 2, 6.1–6.4. Chỗ nào hai bên khác nhau thì file này đúng. Làm xong trước PR-38 (`v0.5.0`).

## Vấn đề

Chạy thử trên server Prominence II thật thì thấy:

- Trang Mod xếp mỗi thứ một hàng: thông báo, nút Thêm, ô kéo thả, nút Kiểm tra cập nhật, rồi mới tới
  bảng. Khi server đang chạy, nút Thêm và ô tải lên đều bị làm mờ, nên người dùng tưởng chỉ còn cách
  lên web tải jar về máy rồi bỏ lên.
- Không cài thêm được mod khi server đang chạy. Thật ra chép thêm một jar mới không ảnh hưởng gì tới
  server: mod chỉ được nạp ở lần khởi động sau.
- Khi chọn mod hay modpack, người dùng chỉ thấy một thẻ một dòng, không đọc được mô tả đầy đủ, không
  có link, phải bấm vào rồi mới chọn được bản. Dự án cũ (mc-controller) có khung chi tiết bên phải.
- Tạo server có mod (Fabric, Forge…) thì không chọn được mod ngay lúc tạo, phải tạo xong rồi vào trang
  Mod cài từng cái một.
- Mỗi lần chỉ cài được một mod, mỗi mod một Job.

## Hành vi

### 1. Trang Mod / Plugin gọn lại

- **Thanh công cụ một hàng** ở đầu bảng (prop `toolbar` của `DataTable`): nút chính "Thêm mod" (hoặc
  "Thêm plugin"), nút "Tải lên .jar" mở hộp chọn file, nút "Kiểm tra cập nhật", ô tìm kiếm của bảng ở
  bên phải. Màn hình hẹp thì xuống hàng như các bảng khác.
- **Bỏ ô kéo thả riêng.** Kéo file `.jar` thả vào bất kỳ chỗ nào của bảng: bảng phủ một lớp "Thả để
  thêm vào server". Nhận nhiều file một lúc. Tiến độ và lỗi của từng file hiện thành dòng nhỏ dưới thanh
  công cụ (như `ContentUpload` hiện nay) và tự ẩn khi xong.
- **Bỏ Notice "Server đang chạy…".** Thay bằng:
  - Công tắc Bật và mục Gỡ, Cập nhật bị khóa, kèm gợi ý khi rê chuột: "Tắt server để bật, tắt hoặc gỡ mod".
  - Khi có mod vừa thêm lúc server đang chạy: một Notice gọn "N mod mới sẽ chạy sau khi khởi động lại",
    kèm nút "Khởi động lại". Hàng của mod đó có nhãn "Chờ khởi động lại".
- Hộp file CurseForge chờ tải tay (spec P3 mục 6.4) giữ nguyên, vẫn chỉ hiện khi có mục chờ.

### 2. Thêm mod khi server đang chạy

- **Làm được khi server đang chạy:** cài từ Modrinth hoặc CurseForge, tải lên jar mới, tải file
  CurseForge chờ tay. File mới vào `mods/` (hoặc `plugins/`). Server đang chạy không nạp file đó; lần
  khởi động sau mới nạp.
- **Vẫn khóa khi server đang chạy:** bật/tắt, gỡ, cập nhật, và mọi thao tác **thay** một jar đã có:
  dependency cần bản mới hơn (`replaces_existing` trong kế hoạch cài). Tải lên file trùng tên vẫn báo
  `FILE_EXISTS` như cũ, không ghi đè. Lý do:
  JVM đang mở các jar này. Trên Windows không đổi tên hay xóa được file đang mở; trên Linux server có
  thể đọc lẫn bản cũ với bản mới khi nạp class muộn. Kế hoạch cài có mục phải thay thì hiện cảnh báo
  "Tắt server để thay bản cũ của X" và nút Cài bị khóa; gọi API thì trả `SERVER_BUSY`.
- Server đang cài (Job tạo server, đổi phiên bản, cập nhật modpack) vẫn khóa mọi thứ như hiện nay.
- Backend: tách `ensure_content_idle` thành hai. `ensure_content_addable` chỉ chặn khi server đang cài;
  `ensure_content_idle` giữ nguyên cho các việc còn lại. `mh-app` giữ trong bộ nhớ danh sách file thêm vào
  lúc server có process, kèm `pid` của process đó; process mới có `pid` khác nên danh sách tự hết hạn. `InstalledContentDto` thêm
  `needs_restart: bool`. Không cần lưu xuống đĩa vì controller tắt thì server cũng tắt theo.
- Kiểm tra trước khi bật (spec P3 mục 5.7) vẫn chạy lại sau mỗi lần thêm. Băng cảnh báo ở đầu trang chỉ
  chặn lần bật sau, không đụng tới server đang chạy.

### 3. Trình duyệt mod (dùng chung ba nơi)

Một khối dùng ở trang thêm mod của server, bước Mod khi tạo server và trang Từ modpack.

- **Màn hình rộng (≥ 1024 px): hai cột.** Cột trái gồm nút chọn nguồn Modrinth / CurseForge (như
  hiện nay), ô tìm, ô sắp xếp (Liên quan, Lượt tải, Mới cập nhật), danh sách kết quả `ProjectCard`
  (thẻ đang chọn được tô) và nút "Tải thêm". Cột phải là chi tiết của project đang chọn; cột này dính
  khi cuộn danh sách.
- **Màn hình hẹp: một cột.** Chọn một kết quả thì chi tiết thay chỗ danh sách, có nút "Quay lại".
- **Khung chi tiết:**
  - Ảnh, tên (link sang trang gốc, mở tab mới), mô tả ngắn, lượt tải, loader. Có nhãn "Chỉ chạy ở
    client" khi `server_side = unsupported`.
  - Ô chọn phiên bản, đã lọc theo loader và Minecraft, mặc định là bản mới nhất. Mỗi dòng ghi số bản,
    ngày và nhãn beta/alpha nếu có.
  - Nút hành động, tùy nơi dùng: "Thêm vào danh sách" (mục 4, 5) hoặc "Dùng pack này" (mục 6).
  - Link Mã nguồn, Wiki, Báo lỗi, Discord (chỉ link nào có).
  - Mô tả đầy đủ.
- **Mô tả đầy đủ:** Modrinth trả markdown. CurseForge trả HTML, lấy thêm qua
  `GET /v1/mods/{modId}/description` và cache như các lời gọi CurseForge khác. UI dùng `marked` để
  chuyển markdown, rồi `DOMPurify` để lọc HTML: chỉ giữ thẻ định dạng, bảng, ảnh và link. Link mở tab
  mới với `rel="noopener noreferrer"`. Bỏ `script`, `style`, `iframe`, form và mọi thuộc tính `on*`.
  Ảnh chỉ nhận `https:`.
- **Sắp xếp:** thêm `sort` (`relevance`, `downloads`, `updated`) vào `GET /content/search`, chuyển sang
  `index` của Modrinth và `sortField` của CurseForge.

### 4. Danh sách chờ cài trên trang Mod

- Trang thêm mod của server là `/servers/:id/content/add`, thay cho hộp thoại `?add=1`. Trang này dùng
  trình duyệt ở mục 3 và có nút "Quay lại danh sách mod".
- "Thêm vào danh sách" ghi bản đã chọn vào danh sách chờ. Thanh dính ở đáy trang hiện "N mod chờ cài ·
  Xem và cài". Bấm vào thì mở hộp thoại kế hoạch: mod chính và dependency được gộp (trùng thì chỉ một
  dòng), mỗi mod chính có nút bỏ, nút "Cài N mod" chạy **một** Job.
- Danh sách chờ lưu ở `localStorage` theo từng server. Đây chỉ là tiện ích trên một trình duyệt, mất cũng
  không sao.
- API: `POST /servers/{id}/content/plan` và `POST /servers/{id}/content` nhận
  `items: [{source, project_id, version_id?}]` (1 đến 50 mục) thay cho một mục. Mỗi mục lập kế hoạch
  theo nguồn của nó rồi gộp lại; một project có mặt hai lần thì giữ bản do người dùng chọn. Job tải hết
  vào thư mục tạm rồi mới chép, nên một mục lỗi thì không file nào được chép (như hiện nay).

### 5. Bước Mod khi tạo server có mod

- Ở `/new`, kiểu "Server có mod" có thêm bước "Mod" nằm giữa Phiên bản và Cài đặt. Bước này dùng trình
  duyệt ở mục 3, lọc theo loader và Minecraft đã chọn. Bước này không bắt buộc: danh sách trống thì nút
  Tiếp ghi "Bỏ qua".
- Mod đã chọn hiện thành danh sách dưới trình duyệt (tên, bản, nút bỏ). Quay lại đổi loader hoặc
  Minecraft thì danh sách bị xóa, kèm một dòng thông báo.
- Chưa có server nên tìm kiếm và danh sách bản nhận `loader`, `minecraft` thay cho `server_id`
  (`GET /content/search`, `GET /content/{source}/projects/{pid}/versions`).
- `POST /servers` nhận `content: [{source, project_id, version_id?}]` (tối đa 50). Job tạo server cài
  loader xong thì lập kế hoạch và cài mod như mục 4, dependency tự thêm. Mod nào không cài được (không
  còn bản hợp, tải hỏng) thì không làm hỏng server: server vẫn được tạo, màn hình tóm tắt của Job liệt
  kê những mod đó.

### 6. Trang Từ modpack

- Trang dùng trình duyệt ở mục 3 với loại modpack. Chọn pack thì khung phải hiện chi tiết và ô chọn
  bản; mỗi bản ghi kèm Minecraft và loader của nó. Nút "Dùng pack này" sang phần tên, RAM, cổng như
  hiện nay.
- Thẻ kéo thả `.mrpack` / `.zip` thu gọn thành nút "Tải lên file pack" cạnh ô tìm. Thả file vào bất kỳ
  chỗ nào của trang vẫn nhận.

## Component

- `FileDrop` thêm `variant="overlay"`: bọc nội dung con, không có khung hay nút riêng, chỉ hiện lớp
  phủ khi đang kéo file vào. Thêm `FileButton` (nút mở hộp chọn file) để đặt nút tải lên vào thanh
  công cụ. `Switch` thêm `title` cho gợi ý khi công tắc bị khóa.
- Thêm `SplitView`: danh sách và chi tiết; hai cột từ 1024 px, cột phải dính khi cuộn; hẹp hơn thì
  một cột.
- Thêm `RichText`: hiện HTML đã lọc theo kiểu chữ của bộ component (tiêu đề, danh sách, bảng, ảnh co
  theo khung, khối code).
- `ProjectCard` thêm `selected`.
- Phần ghép trình duyệt nằm ở `ui/src/pages/catalog/`, dùng chung cho ba trang.

## Phụ thuộc mới

UI thêm `marked` và `dompurify`. Không thêm crate nào.

## Tiêu chí chấp nhận

- Khi server đang chạy, cài một mod mới từ Modrinth thành công, file vào `mods/`, hàng có nhãn "Chờ
  khởi động lại". Khởi động lại thì nhãn mất (test `mh-app`, Vitest).
- Khi server đang chạy, bật/tắt, gỡ, cập nhật và kế hoạch có `replaces_existing` đều trả `SERVER_BUSY`;
  tải lên file trùng tên trả `FILE_EXISTS` (test `mh-app`).
- Kéo hai file `.jar` thả vào bảng thì cả hai được tải lên; file không phải jar thì báo lỗi riêng
  (Vitest).
- Trang Mod không còn ô kéo thả riêng; thanh công cụ nằm trên một hàng khi màn hình ≥ 768 px (xem tay).
- Chọn mod trong trình duyệt thì hiện mô tả đầy đủ. `<script>`, thuộc tính `onerror`, `<iframe>` và ảnh
  `http:` trong mô tả bị bỏ (Vitest).
- Danh sách chờ có 3 mod dùng chung một dependency: kế hoạch gộp, chạy một Job, đủ file (test `mh-api`).
- Tạo server Fabric kèm 2 mod thì được server có loader, 2 mod và dependency của chúng. Nếu một mod
  không còn bản hợp thì server vẫn được tạo và tóm tắt báo mod đó (test `mh-app`, Playwright e2e).
- Trang Từ modpack: chọn pack thì thấy mô tả và ô chọn bản trước khi sang phần cài đặt (Vitest,
  Playwright).
- Ở bề ngang 375 px, trình duyệt hiện một cột và không cuộn ngang (xem tay, cả sáng và tối).

## Không làm

- Bật/tắt, gỡ, cập nhật mod khi server đang chạy. Muốn làm thì cần hàng đợi áp dụng lúc server tắt;
  ghi vào backlog.
- Thư viện ảnh (gallery), changelog từng bản, xem danh sách mod bên trong một modpack.
- Lọc theo danh mục (category).
- Resource pack, datapack, shader.
- Tự khởi động lại server sau khi thêm mod.

## Kế hoạch PR

Đánh số tiếp theo [kế hoạch PR của P3](p3-ke-hoach-pr.md); làm xong hết trước PR-38.

| PR | Việc |
| --- | --- |
| 39 | `docs`: spec này |
| 40 | `feat(app)`: thêm mod khi server đang chạy, `needs_restart` |
| 41 | `feat(api)`: cài nhiều mục trong một Job (`items`), `sort` khi tìm |
| 42 | `feat(app)`: mô tả CurseForge, tìm và danh sách bản theo `loader` + `minecraft`, `POST /servers` nhận `content` |
| 43 | `feat(ui)`: trang Mod gọn, kéo thả vào bảng |
| 44 | `feat(ui)`: trình duyệt mod có khung chi tiết và danh sách chờ |
| 45 | `feat(ui)`: bước Mod khi tạo server |
| 46 | `feat(ui)`: trang Từ modpack dùng trình duyệt |
