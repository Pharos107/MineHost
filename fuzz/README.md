# Fuzz bộ giải nén backup

Target `backup_restore` đưa byte ngẫu nhiên vào `mh_backup::unpack_tar` (tar chưa nén, không
kiểm tra sha256) và báo lỗi nếu có symlink hoặc file nằm ngoài thư mục tạm.

Cần Rust nightly và `cargo-fuzz` trên Linux (libFuzzer không chạy trên Windows):

```bash
cargo install cargo-fuzz
cd fuzz
cargo +nightly fuzz run backup_restore
```

`fuzz/` là workspace riêng, không thuộc `members` của repo. Vì vậy `cargo test` ở gốc không
biên dịch target này.
