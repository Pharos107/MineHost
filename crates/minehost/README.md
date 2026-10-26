# minehost

Binary ghép controller, cấu hình và giao diện. Cách cài cho người dùng nằm ở README gốc của repo.

## Chế độ e2e

Feature `e2e` mặc định tắt. Bản build thường không chứa catalog giả. Khi build với `--features e2e`
và đặt `MINEHOST_E2E_JAVA` tới binary `fake-mc`, controller không gọi mạng: Paper chỉ có `1.21.4`
và `1.20.4`, `server.jar` là file giả, Java là binary đó. Log có một dòng cảnh báo `chế độ e2e`.

Đường dẫn trong biến nên là đường dẫn tuyệt đối. Đường dẫn tương đối được hiểu theo thư mục làm việc
của controller lúc khởi động, vì process server chạy trong thư mục của server.

Trên Linux:

```bash
cargo build -p mh-runtime --bin fake-mc
cargo build -p minehost --features e2e
export MINEHOST_E2E_JAVA="$PWD/target/debug/fake-mc"
./target/debug/minehost --data-dir /tmp/mh-e2e --no-browser --no-tray
```

`cargo build -p minehost --features e2e` cũng tạo `target/debug/e2e-fake-mc` từ cùng mã nguồn
`fake-mc`, để test Rust lấy đường dẫn qua `CARGO_BIN_EXE`. Playwright nên dùng `fake-mc` của
`mh-runtime`.

Không đặt `MINEHOST_E2E_JAVA` khi chạy test Paper thật (`cargo test -p minehost --test paper`).
Bản build không có `--features e2e` bỏ qua biến này.

```bash
cargo test -p minehost --features e2e --test e2e
cargo test -p mh-app --features e2e --lib e2e
```
