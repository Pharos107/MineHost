# Kiểm thử và CI

- Khớp `.github/workflows/` tới: 2026-11-11 (`origin/main` ở `6a03ff8`)
- Lệnh chạy trên máy: [AGENTS.md](../../AGENTS.md#lệnh-thường-dùng).

## CI chạy khi nào

**Chỉ khi có người bấm chạy.** `ci.yml` có duy nhất trigger `workflow_dispatch`; đẩy code hay mở PR
không tự chạy gì. Không có workflow nào chạy theo lịch. `release.yml` chạy khi đẩy tag `v*` và chỉ
build, không chạy test.

Chạy CI cho một nhánh trước khi merge:

```bash
gh workflow run CI --ref <tên-nhánh>
```

```bash
gh run watch
```

Bỏ test tích hợp Paper thật (nhanh hơn vài phút mỗi OS) bằng `-f integration=false`.

## Các job trong `ci.yml`

| Job | Chạy trên | Kiểm tra |
| --- | --- | --- |
| `ui` | Linux | `npm run lint`, `typecheck`, `npm test` (Vitest), `npm run build` |
| `rust` | Windows x64, Linux x64, Linux arm64 | `cargo clippy -D warnings`, `cargo test --workspace`, build release. `cargo fmt --check` chỉ trên Linux x64 |
| `e2e` | Linux | Playwright với binary `minehost` thật ở chế độ e2e (catalog và Java giả, không gọi mạng) |
| `integration` | Windows x64, Linux x64 | `cargo test -p minehost --test paper -- --ignored`: Paper 1.21.4 và Java 21 thật. Bật mặc định, tắt được bằng input `integration` |
| `checks` | Linux | Giới hạn 500 dòng mỗi file mã nguồn; `ui/src/api/openapi.json` khớp `minehost openapi` |
| `deny` | Linux | `cargo-deny`: lỗ hổng, giấy phép, nguồn crate |

## Các lớp test

| Lớp | Ở đâu | Dùng gì | Gọi mạng |
| --- | --- | --- | --- |
| Unit | `src/` và `tests/` của từng crate | `cargo test`, `proptest` cho `server.properties` | Không |
| Provider, tải file | `mh-providers`, `mh-net`, `mh-java` | `wiremock` với fixture JSON ghi từ API thật | Không |
| Runtime, supervisor, app | `mh-runtime`, `mh-supervisor`, `mh-app` | Binary giả `fake-mc` thay cho Java | Không |
| API | `mh-api` | `oneshot` từng route, WebSocket thật trong process | Không |
| Giao diện | `ui/src` | Vitest, Testing Library | Không |
| E2E | `ui/e2e` | Playwright, binary thật ở chế độ e2e ([README của binary](../../crates/minehost/README.md)) | Không |
| Tích hợp | `crates/minehost/tests/paper.rs` | Paper và Java thật; test đánh dấu `#[ignore]` | Có |
| Fuzz | `fuzz/` | `cargo-fuzz`, target `safe_root_join` và `backup_restore`; workflow `fuzz.yml` chạy mỗi Chủ nhật, mỗi target 15 phút ([README](../../fuzz/README.md)) | Không |

Quy tắc: test thường không được gọi mạng thật. Test gọi mạng phải đánh dấu `#[ignore]`.

## Thiếu so với spec

| Spec viết | Thực tế | Việc cần làm |
| --- | --- | --- |
| CI xanh mới được merge | CI chạy tay; không có bảo vệ nhánh nào bắt buộc | Thêm `pull_request` và `push: { branches: [main] }` vào `ci.yml`; bật bảo vệ nhánh `main` |
| Test API thật hằng đêm (spec gốc) | Không có | Workflow `schedule` gọi các test `#[ignore]` của provider |
| Tích hợp Paper thật mỗi PR (bộ rút gọn) và hằng đêm (đầy đủ) | Một bộ, chạy khi bấm tay | Chạy theo lịch nếu không muốn chạy mỗi PR |
| Tích hợp trên Linux arm64 | Chỉ Windows x64 và Linux x64 | Xét khi có người dùng arm64 báo lỗi |
| Chạy dài 72 giờ, 10 server (spec P2) | Chưa làm | PR-34 của P2 |
| Thử tay 5 phút trên Windows sạch (spec P1) | Chưa làm | [Kết quả thử tay](../specs/p1-ket-qua-thu-tay.md) |
