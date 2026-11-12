# Mục lục ADR

Mỗi ADR ghi một quyết định kiến trúc: bối cảnh, quyết định, hệ quả. ADR đã chốt thì không sửa nội
dung; đổi ý thì viết ADR mới và ghi "thay bởi" ở ADR cũ.

| Số | Quyết định | Trạng thái | Ngày |
| --- | --- | --- | --- |
| [0001](0001-server-dung-theo-controller.md) | Server dừng theo controller | Đã chốt | 2026-10-23 |
| [0002](0002-unsafe-trong-binary-cho-khay.md) | Cho phép `unsafe` trong binary cho khay và console | Đã chốt | 2026-10-24 |
| 0003 | Đã đặt mật khẩu thì mở từ chính máy này cũng phải đăng nhập | Nằm trong PR [#44](https://github.com/Pharos107/MineHost/pull/44), chưa merge | 2026-10-26 |
| [0004](0004-chua-dung-sqlite.md) | Chưa dùng SQLite; đĩa là nơi lưu duy nhất | Đã chốt (ghi lại từ spec P1, P2) | 2026-10-23 |
| [0005](0005-dieu-khien-qua-console-khong-rcon.md) | Điều khiển server qua console, không dùng RCON | Đã chốt (ghi lại từ spec P2) | 2026-10-26 |

## Quyết định còn nằm trong spec, chưa tách thành ADR

Đã được duyệt trong spec nhưng chưa có ADR riêng. Tách ra khi có người cần sửa hoặc tranh luận lại.

| Quyết định | Nằm ở |
| --- | --- |
| Chỉ dùng Java do MineHost tải | Spec P1 mục 3.2 |
| Icon khay chỉ có trên Windows | Spec P1 mục 3.3 |
| Xóa server và xóa file là chuyển vào `trash/` | Spec P1 mục 3.4, spec P2 mục 3.6 |
| Phiên bản Minecraft là chuỗi không phân tích | Spec P1 mục 3.5 |
| Giao diện qua LAN chỉ có HTTP ở P2 | Spec P2 mục 3.4 (đang đề nghị xem lại, mục 12 của spec P2) |
| Backup là `tar.zst` kèm file mô tả `.json` | Spec P2 mục 3.7 |
| Dùng `enum` thay cho `dyn Trait` async | Spec gốc, phần Runtime |

## Mẫu

```markdown
# ADR NNNN: <quyết định, viết thành một câu>

- Trạng thái: đề xuất | đã chốt (ngày) | thay bởi ADR NNNN

## Bối cảnh
## Quyết định
## Hệ quả
```
