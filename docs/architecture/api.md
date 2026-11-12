# API: quy ước REST và WebSocket

- Khớp code tới: 2026-11-11 (`origin/main` ở `6a03ff8`)
- Danh sách route, tham số và kiểu dữ liệu **không** ghi ở đây. Nguồn duy nhất là OpenAPI sinh từ code:
  [`ui/src/api/openapi.json`](../../ui/src/api/openapi.json) (in ra bằng `minehost openapi`). CI kiểm
  tra file này khớp backend.

File này ghi những thứ OpenAPI không diễn tả được: quy ước chung, bảo vệ request và giao thức WebSocket.

## Quy ước REST

- Tiền tố `/api/v1`. Body request và response là JSON. Ngoại lệ duy nhất: tải file lên
  (`multipart/form-data`) và tải file xuống (luồng byte).
- Thao tác dài (tạo server, sao lưu) trả `202` kèm `job_id`; theo dõi qua `GET /jobs/{id}` hoặc WebSocket.
- Đường dẫn file đi trong query `?path=`, tính từ `servers/<id>/server/`, không nằm trong path của URL.
- Sửa file văn bản dùng `ETag` và `If-Match`: không khớp thì trả `FILE_CHANGED`.
- Chưa có cam kết tương thích ngược cho `/api/v1`: giao diện và backend luôn phát hành cùng một binary.

## Lỗi

```json
{ "code": "PORT_IN_USE", "params": { "port": 25565, "suggested_port": 25566 } }
```

- `code` là một giá trị của `ErrorCode` (`mh-core/src/error.rs`); HTTP status suy ra từ mã lỗi
  (`mh-api/src/error.rs`).
- Backend không gửi câu chữ. Giao diện dịch theo `code` và ghép `params` vào câu; thêm mã lỗi mới thì
  thêm bản dịch vào `ui/src/i18n/vi.json` và `en.json`.
- Lỗi nội bộ chỉ trả `INTERNAL`; chi tiết nằm trong log của controller.

## Bảo vệ request

Mọi request đi qua `mh-api/src/guard.rs`:

| Kiểm tra | Áp dụng cho | Sai thì |
| --- | --- | --- |
| `Host` là `127.0.0.1:<cổng>` hoặc `localhost:<cổng>` | Mọi request | `403 FORBIDDEN`, `reason: "host"` |
| `Origin` là địa chỉ của MineHost; không có `Origin` thì phải có `X-MineHost: 1` | POST, PUT, PATCH, DELETE và nâng cấp WebSocket | `403 FORBIDDEN`, `reason: "origin"` |
| `Content-Type: application/json` khi có body | POST, PUT, PATCH, DELETE (trừ upload) | `415 INVALID_INPUT` |

Gọi từ dòng lệnh:

```bash
curl -X POST http://127.0.0.1:7878/api/v1/servers/<id>/actions/start -H "X-MineHost: 1"
```

Xác thực bằng mật khẩu (cookie phiên, `X-CSRF-Token`) chưa có trên `main`; mô tả ở
[spec P2 mục 8](../specs/p2-nhieu-server-va-van-hanh.md#8-bảo-mật).

## WebSocket `/api/v1/ws`

Một kết nối dùng chung cho cả giao diện. Client gửi danh sách topic muốn nghe:

```json
{
  "subscribe": ["servers", "jobs", "server.<id>.console", "server.<id>.metrics", "job.<id>"],
  "since": { "server.<id>.console": 120 }
}
```

Server gửi từng tin:

```json
{ "topic": "server.<id>.console", "seq": 121, "data": { } }
```

| Topic | Gửi khi nào | `data` |
| --- | --- | --- |
| `servers` | Một server đổi trạng thái hoặc cấu hình, hoặc bị xóa | `ServerDto`; server bị xóa gửi `{ "id", "removed": true }` |
| `jobs` | Một Job bất kỳ đổi trạng thái hoặc tiến độ, kể cả Job mới | `JobDto` |
| `job.<id>` | Job đó đổi trạng thái hoặc tiến độ | `JobDto` |
| `server.<id>.console` | Có dòng console mới | `ConsoleLineDto` |
| `server.<id>.metrics` | Có mẫu chỉ số mới (mỗi 5 giây khi server chạy) | `MetricSampleDto` |

- Với console, `seq` là số thứ tự của dòng. Kết nối lại và gửi `since` thì nhận tiếp các dòng còn
  trong bộ đệm 5.000 dòng, không mất và không lặp.
- `since` không áp dụng cho `metrics`; lấy lịch sử bằng `GET /servers/{id}/metrics?since=`.
- Các kiểu `…Dto` là kiểu trong OpenAPI, dùng chung với REST.
- Topic sẽ thêm ở P2: `server.<id>.players`.
