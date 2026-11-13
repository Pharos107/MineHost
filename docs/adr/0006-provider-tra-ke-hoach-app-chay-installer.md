# ADR 0006: Provider trả kế hoạch cài, `mh-app` chạy installer của loader

- Trạng thái: đã chốt (2026-11-13 ở spec P3 mục 3.1).

## Bối cảnh

Spec gốc để mỗi provider có hàm `install(ctx, version)` tự chạy installer của loader với Java được
giao. Forge, NeoForge, Quilt và Fabric đều cài server bằng một jar installer, tức là phải chạy một
process Java. `mh-providers` chỉ được phụ thuộc `mh-net`, `mh-core`, `mh-platform` (bảng crate trong
`AGENTS.md`); chạy process là việc của `mh-runtime`, nơi đã có Job Object trên Windows,
`PR_SET_PDEATHSIG` trên Linux và từ P3 là giới hạn RAM, số process.

Nếu provider tự chạy installer thì hoặc thêm cạnh `mh-providers → mh-runtime`, hoặc provider tự dựng
`std::process::Command`, đi vòng qua các lớp bảo vệ của runtime.

## Quyết định

Provider không chạy gì. Mỗi loader trả một `InstallPlan`:

- file cần tải, mỗi file có URL và hash;
- lệnh installer (jar và tham số) nếu loader cần;
- cách chạy server sau khi cài: `Launch::Jar` hoặc `Launch::ArgsFile` (spec P3 mục 3.2).

`mh-app` tải file qua `mh-net`, chạy installer bằng `mh-runtime::run_to_completion` trong Job với Java
đã tải, đọc kết quả thật trên đĩa (ví dụ đường dẫn `args_file`), rồi mới ghi `instance.toml`.

## Hệ quả

- Không thêm cạnh phụ thuộc nào. `mh-providers` vẫn test được hoàn toàn bằng fixture, không cần Java.
- Installer chạy với cùng giới hạn tài nguyên và cùng cách dọn process như server.
- `mh-app` có thêm một bước chung cho mọi loader (tải, chạy installer, kiểm tra kết quả). Loader nào
  không cần installer thì `InstallPlan` không có lệnh, bước đó bỏ qua.
- Lệch với spec gốc phần Providers; chỗ lệch ghi ở `docs/architecture/tong-quan.md`.
