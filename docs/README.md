# Tài liệu MineHost

Mục lục mọi tài liệu kỹ thuật trong repo. Hướng dẫn cho người dùng cuối nằm ở [README gốc](../README.md);
quy tắc code và lộ trình nằm ở [AGENTS.md](../AGENTS.md).

## Quy ước

- Mỗi sự thật chỉ viết ở một nơi. Tài liệu khác dẫn link tới đó, không chép lại.
- Tài liệu mô tả code thì ghi "khớp code tới ngày nào". PR đổi hành vi thì sửa luôn tài liệu đó.
- Spec và ADR có dòng "Trạng thái" ở đầu file. Kế hoạch PR có bảng tiến độ.
- Tên thư mục tiếng Anh, tên file tiếng Việt không dấu, nội dung tiếng Việt.

## Kiến trúc (mô tả code đang có)

| Tài liệu | Nội dung | Khớp code tới |
| --- | --- | --- |
| [architecture/tong-quan.md](architecture/tong-quan.md) | Các lớp, luồng chính, vòng đời controller và server, chỗ lệch với spec gốc | 2026-11-11 |
| [architecture/du-lieu.md](architecture/du-lieu.md) | Thư mục dữ liệu: từng file do ai ghi, mất thì sao | 2026-11-11 |
| [architecture/api.md](architecture/api.md) | Quy ước REST, định dạng lỗi, giao thức WebSocket | 2026-11-11 |
| [architecture/bao-mat.md](architecture/bao-mat.md) | Mô hình mối đe dọa, lớp bảo vệ đang có, lỗ hổng đã biết | 2026-11-11 |

## Quyết định và spec

| Tài liệu | Nội dung |
| --- | --- |
| [adr/README.md](adr/README.md) | Mục lục ADR |
| [specs/p1-mot-server-chay-duoc.md](specs/p1-mot-server-chay-duoc.md) | Spec P1 (đã phát hành `v0.2.0`, còn thiếu lần thử tay) |
| [specs/p1-ke-hoach-pr.md](specs/p1-ke-hoach-pr.md) | Kế hoạch PR của P1 |
| [specs/p1-ket-qua-thu-tay.md](specs/p1-ket-qua-thu-tay.md) | Kết quả thử tay P1 (chưa làm lần chính thức) |
| [specs/p1-ui-components.md](specs/p1-ui-components.md) | Bộ component giao diện |
| [specs/p2-nhieu-server-va-van-hanh.md](specs/p2-nhieu-server-va-van-hanh.md) | Spec P2 (đang làm) |
| [specs/p2-ke-hoach-pr.md](specs/p2-ke-hoach-pr.md) | Kế hoạch PR của P2 kèm bảng tiến độ |
| [backlog.md](backlog.md) | Ý tưởng ngoài giai đoạn hiện tại |
| [ra-soat/2026-11-11-kien-truc.md](ra-soat/2026-11-11-kien-truc.md) | Rà soát kiến trúc: điểm yếu, bằng chứng, việc cần làm |

Spec gốc "MineHost v2 — Spec kiến trúc (Rust)" nằm ngoài repo (link trong AGENTS.md). Chỗ nó lệch với
code ghi ở [architecture/tong-quan.md](architecture/tong-quan.md#chỗ-lệch-với-spec-gốc).

## Vận hành

| Tài liệu | Nội dung |
| --- | --- |
| [ops/kiem-thu-va-ci.md](ops/kiem-thu-va-ci.md) | Lớp test nào có, chạy khi nào, thiếu gì so với spec |
| [ops/phat-hanh.md](ops/phat-hanh.md) | Các bước phát hành một phiên bản |
| [ops/linux-systemd.md](ops/linux-systemd.md) | Chạy MineHost bền trên Linux bằng systemd |
| [../crates/minehost/README.md](../crates/minehost/README.md) | Chế độ e2e của binary |
| [../fuzz/README.md](../fuzz/README.md) | Chạy fuzz bộ giải nén backup |

## Còn thiếu

Chưa viết vì cần quyết định của chủ dự án hoặc chưa có nội dung thật:

- `LICENSE`: `Cargo.toml` đang ghi `license = "UNLICENSED"`. Cần chọn giấy phép trước khi nhận đóng góp.
- `SECURITY.md`: cần một địa chỉ nhận báo cáo lỗ hổng.
- `CHANGELOG.md`: hiện ghi chú phát hành do GitHub tự sinh từ tiêu đề PR.
- Hướng dẫn người dùng cho sao lưu, khôi phục và LAN: viết khi các trang đó merge (PR-34 của P2).
