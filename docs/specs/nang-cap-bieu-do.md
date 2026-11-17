# Spec — Nâng cấp biểu đồ Tổng quan

Trạng thái: Đã duyệt

## Vấn đề

1. Biểu đồ RAM đang khóa cứng trần trục Y bằng `memory_mb.max` (`-Xmx`) và kẹp giá trị bằng `Math.min(v, yMax)`. Khi RSS thực tế của JVM (Heap + ngoài Heap) vượt mức `-Xmx` (ví dụ 12 GB trên 10 GB đã đặt), đường biểu đồ bị cắt ngọn và đi ngang tại trần, khó nhận biết mức dùng thật.
2. Backend chỉ giữ 720 mẫu 5 giây (1 giờ) trong RAM, chưa tùy chỉnh được thời gian giữ lịch sử và không có thao tác zoom/pan hay lọc theo ngày.
3. Nút mở chi tiết hiện là icon nhỏ ở góc phải (`Maximize2`), chưa thuận tiện bằng bấm trực tiếp lên thẻ biểu đồ.

## Hành vi

### 1. Sửa hiển thị trần RAM trên biểu đồ
- Khi có mẫu vượt `yMax` (`memory_mb.max`), trục Y tự mở rộng lên `niceMax(Math.max(yMax, peak))` để đường dữ liệu không bị cắt ngọn.
- Giữ đường tham chiếu nét đứt ngang tại mốc `yMax` (`memory_mb.max`) kèm nhãn mốc để phân biệt giới hạn `-Xmx` đã đặt với RSS thực tế.

### 2. Lưu giữ chỉ số đa tầng trong RAM (tạm trước P5 SQLite)
- Mỗi server có `metrics.retention_minutes` trong `instance.toml` (mặc định `60` phút; bỏ qua khi ghi nếu bằng mặc định; hợp lệ từ `5` phút tới `14_398_560` phút = `9.999` ngày). Đổi qua `PATCH /api/v1/servers/{id}` với trường `metrics_retention_minutes`.
- `MetricsHub` giữ lịch sử theo 4 tầng độ mịn trong RAM:
  - Raw `5 giây`: tối đa `min(retention, 1 giờ)` (720 mẫu).
  - Rollup `1 phút`: tối đa `min(retention, 24 giờ)` (1.440 điểm).
  - Rollup `5 phút`: tối đa `min(retention, 7 ngày)` (2.016 điểm).
  - Rollup `1 giờ`: tối đa `retention` tới `9.999 ngày` (239.976 điểm).
- Mỗi điểm gộp giữ **giá trị trung bình** (`cpu_percent`, `memory_bytes`, `players`) và **giá trị đỉnh** (`cpu_max`, `memory_max_bytes`, `players_max`) để không làm phẳng các pha spike ngắn.
- `GET /api/v1/servers/{id}/metrics` giữ nguyên `since`, bổ sung `until` (RFC 3339) và `max_points` (1..=2000). Response bổ sung `oldest_at`, `retention_minutes`, `step_seconds` và các trường `*_max` tùy chọn trên mỗi mẫu.
- Khi chuyển sang P5 (SQLite), phần lưu giữ trong RAM sẽ được thay bằng bảng lưu bền vững qua lần tắt MineHost (ghi chú ở [backlog](../backlog.md)).

### 3. Trang Tổng quan gọn, bấm thẻ mở chi tiết, bánh răng cài đặt
- **Bấm thẻ để mở chi tiết:** Bỏ icon mũi tên chéo (`Maximize2`). Nhấn bất kỳ vùng nào trên thẻ biểu đồ (trừ nút bánh răng) hoặc kích hoạt tiêu đề bằng bàn phím sẽ mở hộp thoại Xem chi tiết. Không lồng `<button>` bên trong `<button>`.
- **Hiệu ứng mở mượt nhẹ:** Hộp thoại chi tiết nhận `originRect` của thẻ vừa bấm để chạy animation FLIP ngắn bằng `transform` + `opacity` (~200 ms, tôn trọng `prefers-reduced-motion`).
- **Nút bánh răng trên góc thẻ:** Mở hộp thoại cài đặt gồm 2 mục tách biệt, mỗi mục có tiêu đề rõ và biểu tượng chấm than (`InfoTip`) giải thích khi rê chuột:
  1. **Khung hiển thị ngoài Tổng quan** (lưu theo trình duyệt trong `localStorage` cho riêng biểu đồ đó): tối thiểu `5 phút`, tối đa `1 ngày` (`24 giờ`), mặc định `1 giờ`. Có chọn nhanh (`30 phút`, `1 giờ`, `12 giờ`, `1 ngày`) và nhập số + đơn vị (`phút` / `giờ` / `ngày`). Tiêu đề biểu đồ tự đổi theo khung đang chọn (ví dụ `"CPU trong 30 phút qua"`).
  2. **Thời gian lưu dữ liệu trên máy chủ** (lưu ở `instance.toml`, dùng chung cho cả CPU, RAM và Người chơi của server): nhập số + đơn vị (`phút` / `giờ` / `ngày`) trong phạm vi `5 phút` tới `9.999 ngày`, kèm chọn nhanh (`30 phút`, `1 giờ`, `12 giờ`, `1 ngày`). Không hiện cảnh báo ước tính dung lượng.

### 4. Hộp thoại Xem chi tiết
- Chứa toàn bộ công cụ phân tích nâng cao (không đặt ở trang Tổng quan):
  - **Chọn ngày** khi có dữ liệu từ nhiều ngày hoặc lùi xem quá khứ: thanh `< 18/11 >` bấm `<` / `>` chuyển ngày; bấm vào ngày để chọn nhanh từ danh sách các ngày đang có dữ liệu hoặc quay về chế độ `Trực tiếp`.
  - **Chọn nhanh khung thời gian** (`15 phút`, `30 phút`, `1 giờ`, `6 giờ`, `1 ngày`, `Tất cả`) + nút `Đặt lại` khi đang zoom.
  - **Zoom & Pan trực tiếp trên biểu đồ:** kéo chuột bôi vùng để zoom vào đoạn thời gian đó, lăn chuột để zoom vào/ra quanh con trỏ, giữ `Shift` + kéo (hoặc nút trái/phải) để kéo ngang (pan), nhấp đúp để đặt lại.
  - **4 ô tóm tắt** (Hiện tại, Trung bình, Cao nhất, Thấp nhất) tính theo đúng cửa sổ đang xem/zoom; ô *Cao nhất* lấy theo `vMax` nếu là dữ liệu đã gộp.
  - Khi vẽ dữ liệu gộp có `vMax`, hiển thị thêm đường đỉnh mờ và hiện cả giá trị trung bình + đỉnh trong tooltip.

## Tiêu chí chấp nhận

- `mh-core`: kiểm tra biên `5 ..= 14_398_560` phút; `instance.toml` bỏ qua `[metrics]` khi mặc định và đọc/ghi đúng khi khác mặc định (`cargo test -p mh-core`).
- `mh-app` & `mh-api`: lưu đa tầng trong RAM, giữ `*_max` khi rollup, truy vấn `since`/`until`/`max_points` và `PATCH /servers/{id}` đổi `metrics_retention_minutes` (`cargo test -p mh-app -p mh-api`).
- UI:
  - Biểu đồ RAM không cắt ngọn khi vượt `yMax` và vẽ đường tham chiếu mốc đặt (`charts.test.tsx`).
  - Bấm thẻ biểu đồ mở hộp thoại chi tiết (không lồng button), có điều khiển zoom/pan, chuyển ngày `< dd/MM >`, bấm bánh răng mở cài đặt 2 mục có `InfoTip` và cập nhật tiêu đề thẻ (`OverviewPage.test.tsx`).
- Toàn bộ lint, typecheck, `gen:api`, check file size (<= 500 dòng), Rust tests và Vitest đều xanh.

## Không làm

- Lưu lịch sử chỉ số xuống đĩa qua lần tắt MineHost (để dành cho SQLite ở P5).
- Biểu đồ TPS, MSPT, chunk, lưu lượng mạng.
