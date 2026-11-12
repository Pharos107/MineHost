# Phát hành một phiên bản

- Khớp `.github/workflows/release.yml` tới: 2026-11-11

## Workflow làm gì

Đẩy tag dạng `v0.3.0` thì `release.yml`:

1. build giao diện (`npm ci`, `npm run build`);
2. build `minehost` bản release cho ba đích: `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`,
   `aarch64-unknown-linux-gnu`;
3. đóng gói `minehost-<tag>-<đích>.zip` (Windows) hoặc `.tar.gz` (Linux), mỗi gói kèm file `.sha256`;
4. tạo một GitHub Release **dạng nháp**, ghi chú sinh tự động từ tiêu đề PR.

Workflow này **không chạy test** và không ký file. Bản nháp chỉ thành bản công khai khi có người bấm
Publish.

## Các bước

1. `main` đã có mọi PR của phiên bản. Chạy CI trên `main` và chờ xanh, kể cả job tích hợp
   ([kiem-thu-va-ci.md](kiem-thu-va-ci.md)).
2. Tăng số phiên bản ở hai nơi, trong một PR `chore: phát hành vX.Y.Z`:
   - `version` trong `Cargo.toml` ở gốc (rồi `cargo build` để cập nhật `Cargo.lock`);
   - `version` trong `ui/package.json`.
3. Trong cùng PR, sửa tên file và số phiên bản trong [README gốc](../../README.md) (mục "Tải và chạy"
   và lệnh `Get-FileHash`), và mô tả "Bản hiện tại".
4. Merge PR, rồi gắn tag trên `main`:

   ```bash
   git tag vX.Y.Z
   ```

   ```bash
   git push origin vX.Y.Z
   ```

5. Chờ workflow Release xong. Mở bản nháp trên GitHub, kiểm tra đủ 3 gói và 3 file `.sha256`.
6. Tải gói Windows về một máy khác, giải nén, chạy thử: trình duyệt mở giao diện, tạo được một server.
7. Sửa ghi chú phát hành cho người dùng đọc được, rồi bấm Publish.
8. Khi kết thúc một giai đoạn: cập nhật mục "Giai đoạn hiện tại" trong [AGENTS.md](../../AGENTS.md).

## Lưu ý

- Bản Windows chưa ký nên SmartScreen cảnh báo; README gốc có hướng dẫn cho người dùng.
- File `.sha256` nằm cùng chỗ với gói nên chỉ phát hiện lỗi tải, không chứng minh nguồn gốc.
- Chưa có tự cập nhật (P4): người dùng phải tự tải bản mới. Thay binary nghĩa là tắt controller, tức
  là mọi server dừng ([ADR 0001](../adr/0001-server-dung-theo-controller.md)); ghi điều này trong ghi
  chú phát hành.
