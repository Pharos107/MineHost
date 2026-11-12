# Fuzz

Hai target, chạy bằng `cargo-fuzz`:

| Target | Đưa vào | Báo lỗi khi |
| --- | --- | --- |
| `safe_root_join` | Chuỗi ngẫu nhiên vào `SafeRoot::join`, trên một thư mục gốc có sẵn thư mục con, file và symlink trỏ ra ngoài | Đường dẫn được nhận nằm ngoài gốc (theo chữ hoặc sau khi đi theo symlink), hoặc còn `..` |
| `backup_restore` | Byte ngẫu nhiên vào `mh_backup::unpack_tar` (tar chưa nén, không kiểm tra sha256) | Có symlink hoặc file nằm ngoài thư mục tạm |

Workflow `.github/workflows/fuzz.yml` chạy cả hai mỗi Chủ nhật, mỗi target 15 phút; chạy tay được ở
tab Actions → Fuzz với số giây tùy chọn. Tìm ra lỗi thì input làm hỏng được giữ thành artifact.

Chạy trên máy cần Rust nightly và `cargo-fuzz` trên Linux (libFuzzer không chạy trên Windows):

```bash
cargo install cargo-fuzz
cd fuzz
cargo +nightly fuzz run safe_root_join -- -max_total_time=900
cargo +nightly fuzz run backup_restore -- -max_total_time=900
```

Chạy lại một input làm hỏng: `cargo +nightly fuzz run <target> artifacts/<target>/<file>`.

`fuzz/` là workspace riêng, không thuộc `members` của repo. Vì vậy `cargo test` ở gốc không
biên dịch các target này.
