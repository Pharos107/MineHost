# AGENTS.md — quy tắc cho người và AI agent làm việc trong MineHost

MineHost là phần mềm host server Minecraft cho người không rành kỹ thuật. Một binary chạy trên
Windows và Linux, chạy server **trực tiếp bằng Java là mặc định**, Docker chỉ là tùy chọn.

Spec đầy đủ: tài liệu "MineHost v2 — Spec kiến trúc (Rust)" trên Claude
(https://claude.ai/artifact/Np2aG7LQ2r5SpNcuefRyEw). File này tóm những gì phải tuân theo khi code.

Spec gốc đã lệch với code ở vài chỗ; danh sách chỗ lệch nằm ở
[docs/architecture/tong-quan.md](docs/architecture/tong-quan.md). Khi hai bên khác nhau thì spec
giai đoạn trong `docs/specs/` và ADR trong `docs/adr/` là bản đúng. Mục lục mọi tài liệu:
[docs/README.md](docs/README.md).

## Giai đoạn hiện tại: P3 — Mod, plugin và modpack

- [Spec P3](docs/specs/p3-mod-va-modpack.md) (đã duyệt) và
  [kế hoạch PR](docs/specs/p3-ke-hoach-pr.md): 38 PR, nhóm A → H, hai bản phát hành `v0.4.0` (sau
  nhóm D) và `v0.5.0`. Tiến độ ghi ở đầu kế hoạch PR; PR nào merge thì sửa bảng đó trong chính PR ấy.
- Còn mở từ P2: PR-34 (10 server chạy 72 giờ, phát hành `v0.3.0`), xem
  [kế hoạch PR P2](docs/specs/p2-ke-hoach-pr.md). Spec P2: [p2-nhieu-server-va-van-hanh.md](docs/specs/p2-nhieu-server-va-van-hanh.md).
- Còn mở từ P1: lần thử tay 5 phút trên Windows mới cài
  ([kết quả thử tay](docs/specs/p1-ket-qua-thu-tay.md)). Lỗi tìm được ở đó sửa trước việc của P3.
- Quyết định từ P1, P2 vẫn giữ: chưa dùng SQLite (ADR 0004); chỉ dùng Java do MineHost tải; icon khay
  chỉ có trên Windows; xóa server hay file là chuyển vào `trash/`; điều khiển qua console, không RCON.

Chỉ làm việc thuộc giai đoạn hiện tại. Ý tưởng ngoài phạm vi ghi vào
[docs/backlog.md](docs/backlog.md), không code.

| Giai đoạn | Nội dung | Xong khi |
| --- | --- | --- |
| P0 | Workspace, CI, khung API + UI, phát hành | CI xanh trên 3 nền tảng; tải bản release về chạy là mở UI |
| P1 | Paper, tự tải Java, runtime Process, supervisor, console, wizard, EULA, chạy nền có icon khay | Người mới dựng Paper dưới 5 phút trên Windows sạch |
| P2 | Nhiều server, backup/khôi phục, file manager, người chơi, mật khẩu admin, LAN | 10 server chạy 72 giờ; khôi phục backup chơi được |
| P3 | Fabric, Quilt, Forge, NeoForge, Modrinth, CurseForge, modpack | 20 modpack cố định khởi động tới `Done` trong CI hằng đêm |
| P4 | playit.gg, UPnP, Docker, service, Tauri, tự cập nhật | Bạn bè vào từ Internet không cần mở cổng router |
| P5 | Backlog: nhiều tài khoản, audit, MCP, Velocity, metrics | Chọn theo nhu cầu thật |

## Kiến trúc crate

Phụ thuộc chỉ đi từ trên xuống. Không thêm cạnh phụ thuộc nào ngoài bảng này khi chưa sửa spec.

| Crate | Việc | Được phụ thuộc vào |
| --- | --- | --- |
| `minehost` (bin) | Ghép các phần, config, CLI, log | mọi crate |
| `mh-api` | REST/WebSocket `/api/v1`, OpenAPI, nhúng UI | `mh-app`, `mh-core` |
| `mh-app` | Use case, Job, bus sự kiện. Nơi **duy nhất** ghép nhiều crate | các crate bên dưới |
| `mh-supervisor` | Một actor mỗi server: vòng đời, console, crash | `mh-runtime`, `mh-core`, `mh-platform` |
| `mh-backup` | Snapshot, nén, khôi phục | `mh-core`, `mh-platform` |
| `mh-providers` | Mojang, Paper, Fabric, Forge, NeoForge, Modrinth, CurseForge | `mh-net`, `mh-core`, `mh-platform` |
| `mh-java` | Chọn và tải JRE | `mh-net`, `mh-core`, `mh-platform` |
| `mh-runtime` | Chạy process (container từ P4). **Không biết gì về provider** | `mh-core`, `mh-platform` |
| `mh-store` | `instance.toml`, file người chơi của Minecraft (SQLite chưa dùng, ADR 0004) | `mh-core`, `mh-platform` |
| `mh-net` | HTTP chung, retry, cache, kiểm tra hash | `mh-core` |
| `mh-platform` | Thư mục theo OS, `SafePath`, ghi nguyên tử, thông tin máy | `mh-core` |
| `mh-core` | Kiểu domain, mã lỗi. **Không I/O, không async, không phụ thuộc crate nội bộ** | — |

Quyết định đã chốt:

- Server là process con và **dừng theo controller**. Controller chạy nền, chỉ thoát qua nút
  "Thoát" an toàn (gửi `stop` tới từng server). Khi controller crash, OS kill process con.
- Thư mục server trên đĩa (`instance.toml`) là nguồn dữ liệu chính; SQLite chỉ là chỉ mục.
- Mặc định chỉ lắng nghe `127.0.0.1`. Mở ra LAN bắt buộc có mật khẩu admin (từ P2).
- Controller không bao giờ tự ghi `eula=true`; người dùng phải tự bấm đồng ý.
- Không hỗ trợ macOS ở v2.0. Bản đầu chưa ký code Windows.

## Quy tắc code

- Mỗi file tối đa **500 dòng**, mỗi hàm tối đa khoảng 60 dòng. `scripts/check-file-size.sh` chặn trong CI.
- Không `unwrap()`, `expect()`, `panic!()` ngoài test; clippy đã đặt `deny`.
- Thư viện dùng `thiserror`; `anyhow` chỉ dùng trong bin `minehost`.
- `unsafe` bị cấm ở cấp workspace. Chỉ mở bằng `#![allow(unsafe_code)]` trong vài module nhỏ, mỗi
  khối có comment `// SAFETY:`: `mh-runtime/src/os/` (Job Object, `prctl`) và `minehost/src/win32.rs`
  (console, vòng lặp message của khay; xem `docs/adr/0002-unsafe-trong-binary-cho-khay.md`).
- Mọi đường dẫn nhận từ người dùng đi qua `SafeRoot::join`. Mọi file cấu hình ghi bằng
  `mh_platform::fs::write_atomic`.
- Lỗi gửi cho client dùng `ErrorCode` (`mh-core`) và `ApiError` (`mh-api`). Backend không gửi
  câu chữ đã dịch; thêm mã lỗi mới thì thêm bản dịch trong `ui/src/i18n/vi.json` và `en.json`.
- Comment và thông báo log viết tiếng Việt, ngắn, giải thích *vì sao* chứ không lặp lại code.
- Mọi thay đổi hành vi phải có test. Test không được gọi mạng thật (dùng fixture).

## Frontend (`ui/`)

- React, Vite, TypeScript strict, TanStack Query, i18next (vi mặc định, en).
- Kiểu dữ liệu API sinh từ OpenAPI: sửa DTO trong `mh-api` rồi chạy `npm run gen:api`,
  commit cả `src/api/openapi.json` và `src/api/schema.d.ts`. CI kiểm tra hai file này khớp backend.
- Không hard-code chuỗi hiển thị; mọi chuỗi đi qua `t(...)`.
- Trang chỉ ghép từ bộ component trong `ui/src/ui/` (spec `docs/specs/p1-ui-components.md`); không tự
  đặt màu, cỡ chữ, khoảng cách, không ghi đè style của component. Thiếu kiểu thì thêm `variant` vào
  component. Xem mọi component tại `http://localhost:5173/#/ui` khi chạy `npm run dev`.
- TypeScript giữ ở 5.9 vì `typescript-eslint` và `openapi-typescript` chưa hỗ trợ TypeScript 7.

## Lệnh thường dùng

```bash
# Build UI trước để binary nhúng được giao diện
cd ui && npm ci && npm run build && cd ..

# Chạy controller, dữ liệu để trong thư mục tạm của repo
cargo run -p minehost -- --data-dir .minehost-dev

# Kiểm tra giống CI
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/check-file-size.sh
cd ui && npm run lint && npm run typecheck && npm test

# Test E2E bằng Playwright với binary thật ở chế độ e2e (catalog và Java giả, không cần mạng)
cd ui && npm run build && cd ..
cargo build -p mh-runtime --bin fake-mc && cargo build -p minehost --features e2e
cd ui && npx playwright install chromium && npm run e2e

# Test tích hợp với Paper thật (tải Java và Paper, vài phút; CI chạy trong job `integration`)
cargo test -p minehost --test paper -- --ignored --test-threads=1
```

Khi phát triển UI: chạy controller như trên, rồi `cd ui && npm run dev` (Vite chuyển `/api` tới cổng 7878).

## Quy trình

- Mỗi tính năng có spec ngắn trong `docs/specs/` trước khi code: vấn đề, hành vi, tiêu chí chấp nhận, không làm.
- Mỗi PR một việc, khoảng 400 dòng trở xuống (không tính code sinh tự động). CI xanh mới merge.
  CI hiện **chỉ chạy tay** (`workflow_dispatch`): trước khi merge phải tự chạy workflow CI trên nhánh
  của PR và chờ xanh. Lớp test nào chạy khi nào: [docs/ops/kiem-thu-va-ci.md](docs/ops/kiem-thu-va-ci.md).
- Commit theo Conventional Commits (`feat:`, `fix:`, `chore:`, `docs:`, `test:`, `refactor:`).
- Quyết định kiến trúc quan trọng ghi thành ADR trong `docs/adr/` và thêm một dòng vào
  [mục lục ADR](docs/adr/README.md).
- Mỗi sự thật chỉ viết ở một nơi; tài liệu khác dẫn link tới đó. PR đổi hành vi thì sửa luôn tài liệu
  mô tả hành vi ấy.
