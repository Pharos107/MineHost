# Spec P3 — Mod, plugin và modpack

- Trạng thái: **bản nháp, chờ duyệt.** Chưa code gì của P3 trước khi spec này merge.
- Ngày: 2026-11-13
- Spec gốc: "MineHost v2 — Spec kiến trúc (Rust)", phần Providers, Modpack và Lộ trình P3
- Kế hoạch PR: [p3-ke-hoach-pr.md](p3-ke-hoach-pr.md)
- Tham khảo: dự án cũ mc-controller (`Server MC Host/internal/modrinth`, `curseforge`, `missingmods`)

## 1. Mục tiêu

Người dùng dựng được server có mod hoặc chơi một modpack mà không phải biết loader là gì, không phải
tải tay từng file và không phải đọc log để đoán vì sao server không lên. Cụ thể:

- Tạo server Fabric, Quilt, Forge, NeoForge như tạo server Paper ở P1.
- Tìm và cài mod (server có loader) hoặc plugin (Paper, Purpur) từ Modrinth và CurseForge, kèm
  dependency.
- Cài modpack từ Modrinth (`.mrpack`) và CurseForge, bằng tìm kiếm hoặc kéo thả file. MineHost tự
  nhận loader và phiên bản Minecraft.
- Mod chỉ chạy ở client tự bị tắt. Thiếu dependency thì báo trước khi bật, không phải đợi server crash.
- Cập nhật mod, cập nhật modpack và đổi phiên bản Minecraft, luôn có backup trước.

### Tiêu chí hoàn thành

- [ ] 20 modpack cố định (mục 9.2), cả Modrinth và CurseForge, đủ bốn loader, cài và khởi động tới
      `Done` trong workflow `modpacks` chạy hằng đêm trên runner tự host.
- [ ] Mod bị CurseForge cấm phân phối được liệt kê rõ kèm link tải. Người dùng kéo file vào thì cài
      tiếp được, server bật được (test Playwright với nguồn giả và test tích hợp với một pack thật).
- [ ] Mỗi loader có test tích hợp trong job `integration`: tạo server, bật tới `Done`, tắt sạch, trên
      Windows và Linux.
- [ ] Cập nhật một modpack lên bản mới: có bản backup `before_update`; khôi phục bản đó thì server quay
      về bản cũ và bật được.
- [ ] Server vượt giới hạn RAM của MineHost bị dừng và hiện lý do "vượt giới hạn RAM", không làm
      treo máy (test với `fake-mc` trên Windows và Linux có cgroup).
- [ ] Mỗi trang mới ở mục 6 có ít nhất một test Playwright chạy với API thật.

### Không làm trong P3

Docker, tunnel, UPnP, truy cập từ Internet (P4); Hangar, SpigotMC (P5); Folia, Velocity, BungeeCord;
modpack của FTB App, Technic, ATLauncher; mod cho Minecraft cũ hơn 1.12.2; NeoForge 1.20.1 (tọa độ
`net.neoforged:forge`); chặn mod đọc file của user (Landlock, AppContainer: vẫn ở backlog); TPS, MSPT;
"bác sĩ server" đọc `crash-reports/`; preset cờ JVM; tự cập nhật mod theo lịch.

## 2. Bố cục và điều hướng

| Ngữ cảnh | Thêm | Đường dẫn |
| --- | --- | --- |
| Trong một server | Mod (server có loader) hoặc Plugin (Paper, Purpur); Vanilla không có mục này | `/servers/:id/content` |
| | Hộp thoại tìm và cài | `/servers/:id/content?add=1` |
| | Cập nhật và đổi phiên bản, nằm trong Cài đặt server | `/servers/:id/settings#version` |
| Tạo máy chủ | Bước 1 có ba lựa chọn: Server thường, Server có mod, Từ modpack | `/new`, `/new/modpack` |
| Cài đặt chung | Mục CurseForge: trạng thái key, ô nhập key riêng | `/settings#curseforge` |

- Thẻ server ở Trang chủ và trang Máy chủ hiện thêm tên loader và tên modpack (nếu có).
- Đầu trang server hiện băng cảnh báo khi có việc chặn bật server (mục 5.7): thiếu dependency, còn
  file CurseForge chưa tải. Băng có nút dẫn tới trang Mod.

## 3. Quyết định riêng của P3

### 3.1 Provider trả kế hoạch cài, `mh-app` chạy installer

Spec gốc để provider tự chạy installer. Như vậy `mh-providers` phải chạy process, tức thêm cạnh phụ
thuộc tới `mh-runtime`. P3 làm khác: provider chỉ trả `InstallPlan` (file cần tải kèm hash, lệnh
installer nếu có, cách chạy server). `mh-app` tải file, chạy installer qua `mh-runtime` trong Job, rồi
ghi `instance.toml`. Không thêm cạnh phụ thuộc nào. Ghi lại thành ADR 0006.

Mọi loader đều cài trong Job, kể cả Fabric (dùng installer chính thức thay vì launcher tự tải thư viện
lúc bật lần đầu). Lần bật đầu tiên vì thế không cần mạng và không chậm hơn các lần sau.

| Loader | Nguồn phiên bản | Cài | Cách chạy |
| --- | --- | --- | --- |
| Fabric | `meta.fabricmc.net/v2` | `fabric-installer.jar server -mcversion <mc> -loader <v> -downloadMinecraft` | `jar`: `fabric-server-launch.jar` |
| Quilt | `meta.quiltmc.org/v3` | `quilt-installer.jar install server <mc> <v> --download-server --install-dir=.` | `jar`: `quilt-server-launch.jar` |
| Forge | `maven.minecraftforge.net`, `promotions_slim.json` | `forge-<mc>-<v>-installer.jar --installServer` | Dưới 1.17: `jar` (`forge-<mc>-<v>.jar`); từ 1.17: `args_file` |
| NeoForge | `maven.neoforged.net`, `maven-metadata.xml` | `neoforge-<v>-installer.jar --installServer` | `args_file` |

- Tham số dòng lệnh trong bảng ghi theo tài liệu hiện có của từng installer; PR của loader đó kiểm
  tra lại bằng test tích hợp (PR-11) trước khi chốt.
- Installer chạy bằng Java mà server sẽ dùng, trong thư mục `server/`, tối đa 15 phút, log vào Job.
  Mã thoát khác 0 thì trả `LOADER_INSTALL_FAILED` kèm 40 dòng log cuối.
- Phiên bản loader mặc định: bản stable mới nhất cho phiên bản Minecraft đã chọn (Forge: bản
  `recommended`, không có thì `latest`). Chế độ Nâng cao cho chọn bản khác.
- Jar installer kiểm tra hash từ file `.sha256` (không có thì `.sha1`) cạnh nó trên maven. Hash và jar
  cùng một nguồn, nên việc này chỉ chặn file hỏng, không chặn maven bị chiếm quyền (mục 8).
- Thứ tự phiên bản Minecraft vẫn lấy từ manifest của Mojang. NeoForge đổi cách đánh số từ 26.1
  (`21.4.140` là 1.21.4, `26.1.2.76` là 26.1.2); ánh xạ có fixture cho cả hai kiểu.

### 3.2 Hai cách chạy server: `jar` và `args_file`

Forge và NeoForge từ 1.17 chạy bằng `java @user_jvm_args.txt @libraries/.../unix_args.txt` thay vì
`-jar`. MineHost không gọi `run.sh`, `run.bat`, cũng không đọc `user_jvm_args.txt`: RAM và cờ JVM vẫn
lấy từ `instance.toml`. Lệnh chạy là
`<java> -Xms -Xmx <encoding> <jvm_args> @<args_file> nogui`, trong đó `<args_file>` là
`unix_args.txt` hoặc `win_args.txt` tùy OS. Nhờ vậy server tạo trên Linux chép sang Windows vẫn chạy.

Có `user_jvm_args.txt` chứa `-Xmx` (thường gặp khi nhập server có sẵn) thì trang Cài đặt server báo
rằng MineHost bỏ qua file này.

### 3.3 Đĩa là nguồn chính của danh sách mod

Danh sách mod là những gì đang nằm trong `mods/` (hoặc `plugins/`) và `mods-disabled/`. File
`servers/<id>/content.toml` (mục 4.2) chỉ ghi thêm nguồn gốc của từng file để cập nhật được. Người
dùng chép jar vào bằng tay hay qua trang Tệp thì jar đó hiện là "Thêm tay"; xóa jar bằng tay thì dòng
của nó trong `content.toml` bị bỏ ở lần đọc sau. Không file nào bị coi là đã cài chỉ vì có trong
`content.toml`.

Tắt một mod là chuyển jar từ `mods/` sang `mods-disabled/` (cùng tên). Gỡ mod là chuyển vào thùng rác
như trang Tệp (spec P2, mục 3.6). Chặn bật, tắt, gỡ mod khi server đang chạy (`SERVER_BUSY`).

### 3.4 File tải về lưu theo hash, dùng chung giữa các server

Mọi file mod, plugin, installer lưu ở `cache/files/<sha1[0..2]>/<sha1>` sau khi kiểm tra hash, rồi
chép sang thư mục server. Modpack 300 mod cài cho server thứ hai không phải tải lại. Cache không tự
dọn trong P3; trang Cài đặt hiện dung lượng và có nút "Xóa bộ nhớ đệm tải về".

### 3.5 Mod chỉ chạy ở client tự bị tắt

Một file bị coi là chỉ chạy ở client khi một trong các nguồn sau nói vậy, theo thứ tự ưu tiên:

1. `env.server = "unsupported"` trong `.mrpack`: file không được tải, ghi vào danh sách "bỏ qua".
2. `server_side = "unsupported"` của project trên Modrinth.
3. Metadata trong jar: `"environment": "client"` trong `fabric.mod.json`;
   `minecraft.environment = "client"` trong `quilt.mod.json`; trong `META-INF/mods.toml` hoặc
   `neoforge.mods.toml`, mọi dependency vào `minecraft`, `forge` hoặc `neoforge` đều có
   `side = "CLIENT"` (cách của dự án cũ, `missingmods/jarmeta.go`).

File loại 2 và 3 được cài vào `mods-disabled/` với nhãn "Chỉ chạy ở client". Người dùng bật lại được;
MineHost không tắt lại file người dùng đã tự bật. CurseForge không có trường tương đương nên mod từ
CurseForge chỉ dựa vào nguồn 3.

### 3.6 CurseForge dùng key riêng của MineHost

- Chủ dự án đăng ký key cho ứng dụng trên console của CurseForge. Key nằm trong secret
  `MINEHOST_CURSEFORGE_KEY` của GitHub, được nhúng vào binary lúc build bản phát hành
  (`option_env!`). Bản tự build không có key thì CurseForge tắt, giao diện ghi rõ lý do.
- Người dùng có thể nhập key của chính họ ở Cài đặt; key này ghi vào `<data>/secrets.toml` (mục 4.3)
  và được dùng thay key nhúng. API không bao giờ trả key về giao diện, chỉ trả `source`.
- Mod có `allowModDistribution = false` hoặc file không có `downloadUrl` không tải được qua API.
  MineHost không tìm cách vượt qua: cài phần còn lại, liệt kê các file này kèm link tới trang tải trên
  CurseForge, rồi nhận file người dùng kéo vào nếu SHA-1 khớp hash mà API trả. Trong lúc còn thiếu
  file, server không bật được trừ khi người dùng chọn "Bật mà không có các mod này".
- Pack có server pack chính thức (`serverPackFileId`) thì dùng server pack; không có thì dựng từ
  `manifest.json`.

### 3.7 Chỉ tải từ host đã biết

`mh-net` nhận thêm danh sách host được phép cho mỗi nguồn. URL ngoài danh sách bị từ chối
(`DOWNLOAD_HOST_NOT_ALLOWED`), kể cả URL nằm trong file `.mrpack` người dùng tải lên.

| Nguồn | Host |
| --- | --- |
| Modrinth và `.mrpack` | `cdn.modrinth.com`, `github.com`, `raw.githubusercontent.com`, `gitlab.com` (danh sách của Modrinth); `objects.githubusercontent.com` (nơi `github.com` chuyển hướng tới) |
| CurseForge | `edge.forgecdn.net`, `mediafilez.forgecdn.net` |
| Loader | `meta.fabricmc.net`, `maven.fabricmc.net`, `meta.quiltmc.org`, `maven.quiltmc.org`, `maven.minecraftforge.net`, `files.minecraftforge.net`, `maven.neoforged.net` |

Installer của loader tự tải thư viện từ maven của nó và của Mojang. MineHost không kiểm soát được
phần này (mục 8).

### 3.8 Giới hạn tài nguyên của process server (rà soát A1, phần đầu)

Mod và plugin là code tùy ý. P3 làm phần chắc chạy được với JVM: giới hạn RAM và số process. Chặn đọc
file vẫn ở backlog.

- Giới hạn RAM của một server là `max × 1,5 + 1024` MB (`max` là `-Xmx`). Lần chạy dài của P2 đo
  được Paper nhàn rỗi với `-Xmx512M` chiếm khoảng 1 GB RSS, tức gấp đôi heap. Công thức trên chừa đủ
  cho trường hợp đó và cho modpack lớn. Cần đo lại trong workflow modpack (mục 9.2).
- **Windows:** đặt thêm `JobMemoryLimit` và `ActiveProcessLimit = 16` vào Job Object đang có
  (`mh-runtime/src/os/windows.rs`).
- **Linux:** nếu controller có cgroup v2 ghi được (chạy dưới `systemd-run --user`, hoặc unit có
  `Delegate=yes`), mỗi server có một cgroup con với `memory.max` như trên, `memory.swap.max = 0` và
  `pids.max = 2048` (`pids` đếm cả luồng, server có mod dùng vài trăm luồng). Không ghi được thì chạy
  như P2 và ghi cảnh báo một lần vào log; trang Cài đặt hiện "Chưa giới hạn được RAM trên máy này".
- Server bị OS kill vì vượt giới hạn thì `CrashInfo` có lý do mới `memory_limit`, không tự bật lại
  (bật lại cũng sẽ bị kill tiếp).

### 3.9 Cập nhật và đổi phiên bản luôn có backup trước

Cập nhật mod, cập nhật modpack, đổi phiên bản loader hay Minecraft đều là Job: tắt server nếu đang
chạy (hộp thoại đã báo) → backup loại mới `before_update` (bảo vệ, không tính vào `keep`) → làm thay
đổi → không tự bật lại. Lỗi giữa chừng thì Job báo lỗi và chỉ người dùng tới bản backup đó; không tự
khôi phục, vì khôi phục cũng có thể lỗi và người dùng cần thấy chuyện gì đã xảy ra.

Đổi sang phiên bản Minecraft cũ hơn hiện cảnh báo đỏ: world đã mở bằng bản mới thường không mở lại
được bằng bản cũ. Đổi loại loader (ví dụ Fabric sang Forge) không làm trong P3.

Cùng luồng này lo luôn việc cập nhật build Paper và Purpur cho server đã tạo (rà soát B4).

### 3.10 Chọn Java cho từng server

`game.java_major` vẫn lấy theo Mojang khi tạo. Trang Cài đặt server (chế độ Nâng cao) cho chọn major
khác trong các bản Adoptium có, kèm cảnh báo khi khác bản Mojang yêu cầu. Lý do: modpack cũ đôi khi
cần đúng một bản Java (rà soát B1).

### 3.11 Kiểm thử modpack trên runner tự host

Repo private nên runner của GitHub chỉ có khoảng 7 GB RAM và giới hạn phút chạy. Workflow `modpacks`
chạy trên một runner tự host trên máy Linux thử nghiệm, lúc 04:00 giờ Việt Nam, mỗi lần một pack.
Runner chạy dưới unit systemd riêng có `MemoryMax=12G`, `CPUWeight=20`, `Nice=10` để không ảnh hưởng
dịch vụ khác trên máy. Cách cài runner ghi ở `docs/ops/runner-modpack.md` (PR-36).

## 4. Mô hình dữ liệu

### 4.1 Thêm vào `instance.toml` (vẫn schema 1)

```toml
[game]
loader = "neoforge"            # thêm: fabric | quilt | forge | neoforge
minecraft = "1.21.4"
loader_version = "21.4.140"    # mới; Paper, Purpur vẫn dùng `build`
java_major = 21
jar = "server.jar"             # vẫn bắt buộc; kiểu `args_file` không dùng tới

[game.launch]                  # mới, tùy chọn; thiếu bảng này nghĩa là `jar`
kind = "args_file"
args_file = "libraries/net/neoforged/neoforge/21.4.140/{os}_args.txt"   # {os} = unix | win

[modpack]                      # mới, chỉ có khi tạo từ modpack
source = "modrinth"            # modrinth | curseforge | file
project_id = "1KVo5zza"
version_id = "aBcD1234"
name = "Fabulously Optimized"
version_name = "6.4.0"
```

Bản P2 đọc file có `loader = "fabric"` sẽ đánh dấu server `Broken` vì không biết loại này. Đó là hành
vi đúng: P2 không chạy được server có mod.

### 4.2 `servers/<id>/content.toml`

Nằm cạnh `instance.toml`, không nằm trong `server/`, để trang Tệp và backup của world không đụng tới.
Backup vẫn chép bản chụp file này vào archive để khôi phục được cả nguồn gốc mod. Ghi bằng
`write_atomic`; khóa lạ được giữ.

```toml
schema = 1

[[file]]
path = "mods/sodium-fabric-0.6.0.jar"   # tương đối với server/, kể cả khi đang ở mods-disabled/
sha1 = "…"
source = "modrinth"                     # modrinth | curseforge | modpack | upload
project_id = "AANobbMI"
version_id = "…"
client_only = false
from_modpack = true
installed_at = "2026-11-13T10:00:00Z"

[[pending]]                             # file CurseForge chưa tải được (mục 3.6)
path = "mods/optifine.jar"
project_id = 238222
file_id = 4912345
sha1 = "…"
page_url = "https://www.curseforge.com/minecraft/mc-mods/…/download/4912345"
```

### 4.3 `<data>/secrets.toml`

```toml
curseforge_api_key = "…"   # tùy chọn, ghi đè key nhúng
```

Quyền file `0600` trên Linux. Không nằm trong backup, không hiện trong trang Tệp.

### 4.4 Mã lỗi mới trong `ErrorCode`

| Mã | Khi nào |
| --- | --- |
| `LOADER_INSTALL_FAILED` | Installer của loader thoát với mã khác 0 hoặc quá 15 phút |
| `CONTENT_INCOMPATIBLE` | Mod hoặc plugin không có bản cho loader và phiên bản Minecraft của server |
| `CONTENT_NOT_FOUND` | Project hoặc file không còn trên Modrinth hoặc CurseForge |
| `DEPENDENCY_MISSING` | Bật server khi còn thiếu dependency bắt buộc, kèm danh sách |
| `CURSEFORGE_UNAVAILABLE` | Không có key, hoặc key bị từ chối |
| `CURSEFORGE_FILES_PENDING` | Bật server khi còn file CurseForge chưa tải tay |
| `UPLOAD_HASH_MISMATCH` | File kéo vào không khớp hash của file đang chờ |
| `MODPACK_INVALID` | Thiếu `modrinth.index.json` hoặc `manifest.json`, sai định dạng, đường dẫn thoát thư mục |
| `DOWNLOAD_HOST_NOT_ALLOWED` | URL tải nằm ngoài danh sách ở mục 3.7 |
| `UPSTREAM_RATE_LIMITED` | Modrinth hoặc CurseForge trả 429, kèm `retry_after_s` |

`MODS_NOT_SUPPORTED` (P2) chỉ còn dùng khi nhập server có `mods/` mà không nhận ra loader.

## 5. Thiết kế từng crate

Không thêm cạnh phụ thuộc mới giữa các crate.

### 5.1 `mh-core`

- `LoaderKind` thêm `Fabric`, `Quilt`, `Forge`, `NeoForge`; `fn content_kind(loader) -> Mods | Plugins | None`.
- `Launch { Jar(JarName), ArgsFile(RelPath) }` và hàm thuần thay `{os}`.
- Parser thuần trên chuỗi (không đọc file): `fabric.mod.json`, `quilt.mod.json`, `mods.toml`,
  `neoforge.mods.toml`, `plugin.yml`, `paper-plugin.yml` → `ModMeta { id, name, version, side, depends }`.
- `MrpackIndex` (serde của `modrinth.index.json`), `CfManifest` (serde của `manifest.json`), kiểm tra
  `path` không chứa `..`, không tuyệt đối.
- Ánh xạ phiên bản NeoForge ↔ Minecraft; so khớp ràng buộc phiên bản đơn giản trong `depends`
  (`>=`, `<`, `*`, khoảng của Maven) chỉ để báo thiếu hoặc sai bản, không chặn cài.
- `fn memory_limit_mb(max) -> u64` (mục 3.8), các mã lỗi ở mục 4.4.
- `mh-backup` thêm `BackupKind::BeforeUpdate` (mục 3.9) và chép `content.toml` vào archive (mục 4.2).

### 5.2 `mh-platform`

- `fn cgroup_v2_writable() -> Option<PathBuf>` (Linux): cgroup hiện tại của process và quyền ghi.

### 5.3 `mh-net`

- `Checksum::Sha1`, `Checksum::Sha512`.
- `HostAllowlist` gắn vào từng lần tải (mục 3.7); kiểm tra cả sau mỗi lần chuyển hướng.
- User-Agent chung `MineHost/<phiên bản> (+https://github.com/Pharos107/MineHost)`; Modrinth bắt buộc
  có User-Agent riêng.
- Xử lý 429: đọc `Retry-After` hoặc `X-Ratelimit-Reset`, trả `UPSTREAM_RATE_LIMITED`, không tự thử lại
  quá 1 lần.

### 5.4 `mh-providers`

- `fabric`, `quilt`, `forge`, `neoforge`: `versions(mc)`, `plan(mc, loader_version) -> InstallPlan`.
- `modrinth`: `search(query, filters)`, `project(id)`, `versions(project, loader, mc)`,
  `version(id)`, `versions_by_hash(sha1[])` (để nhận ra mod "thêm tay" và kiểm tra cập nhật).
  Cache 10 phút cho tìm kiếm, 1 giờ cho project.
- `curseforge`: `search`, `mod(id)`, `files(mod, loader, mc)`, `files_by_id[]`. Game `432`; lớp
  `6` (mod), `4471` (modpack), `5` (plugin Bukkit, chỉ dùng để tìm, không cài trong P3: xem câu hỏi
  mở ở mục 11).
- `modpack`: đọc `.mrpack` và `.zip` của CurseForge bằng `zip`, giải nén `overrides/` rồi
  `server-overrides/` qua `SafeRoot`, bỏ symlink. Trả `PackPlan { loader, minecraft, loader_version,
  files, skipped_client, blocked }`.

### 5.5 `mh-runtime`

- `run_to_completion(spec, timeout, on_line) -> ExitStatus`: chạy installer, cùng Job Object hoặc
  cgroup như server, log từng dòng.
- `JavaLaunch` nhận `Launch` thay vì `JarName`.
- Windows: thêm `JobMemoryLimit`, `ActiveProcessLimit` (mục 3.8). Linux: tạo cgroup con trước `exec`,
  ghi `cgroup.procs` trong `pre_exec` (khối `unsafe` có `// SAFETY:` trong `os/linux.rs`).
- Phân biệt "bị kill vì vượt RAM": Windows đọc thông báo `JOB_OBJECT_MSG_JOB_MEMORY_LIMIT` qua
  completion port; Linux đọc `memory.events` (`oom_kill` tăng).

### 5.6 `mh-store`

- Đọc ghi `content.toml` và `secrets.toml`.
- `scan_content(server_dir, kind) -> Vec<ContentFile>`: liệt kê jar trong `mods/`, `mods-disabled/`
  (hoặc `plugins/`), đọc metadata bằng `zip` và parser của `mh-core`, tính SHA-1, ghép với
  `content.toml`. Cache theo `(path, mtime, size)` trong RAM để mở trang Mod không phải đọc lại 300 jar.

### 5.7 `mh-app`

- **Tạo server có loader:** Job tải Java, tải installer, chạy installer, ghi `instance.toml`.
- **Cài từ modpack:** Job đọc pack (tải về hoặc file tải lên) → tạo server với loader của pack →
  tải song song tối đa 6 file, kiểm tra hash, chép từ cache → áp overrides → ghi `content.toml` →
  kết quả kèm số file bỏ qua vì chỉ chạy client và danh sách file CurseForge còn chờ.
  RAM đề xuất cho modpack: `max` = 1/2 RAM máy, kẹp trong 4096..=10240 MB.
- **Cài mod:** chọn bản hợp loader và phiên bản; tự cài dependency bắt buộc (Modrinth
  `dependency_type = required`, CurseForge `relationType = 3`); trùng mod đã có thì thay bản cũ.
- **Kiểm tra trước khi bật** (`preflight`): thiếu dependency bắt buộc (so `depends` trong metadata với
  id các mod đang bật), sai loader, file CurseForge còn chờ. Có lỗi thì `start` trả
  `DEPENDENCY_MISSING` hoặc `CURSEFORGE_FILES_PENDING`, trừ khi gửi `{ "ignore_preflight": true }`.
- **Cập nhật:** `check_updates` hỏi Modrinth theo hash (`versions_by_hash`) và CurseForge theo file id;
  `update(items)` và `change_version(minecraft?, loader_version?)` đi theo mục 3.9.
- **Nhập server có sẵn:** nhận ra Fabric (`.fabric/`, `fabric-server-launch.jar`), Quilt, Forge
  (`libraries/net/minecraftforge`), NeoForge (`libraries/net/neoforged`); không còn trả
  `MODS_NOT_SUPPORTED` cho bốn loại này.

### 5.8 `mh-api`

| Method | Path | Ghi chú |
| --- | --- | --- |
| GET | `/catalog/{loader}/versions` | Thêm bốn loader mới |
| GET | `/catalog/{loader}/{minecraft}/loader-versions` | Bản loader, bản đề xuất đứng đầu |
| GET | `/content/search?source=&kind=&q=&server_id=&page=` | `kind` là `mod`, `plugin`, `modpack`; có `server_id` thì lọc theo loader và phiên bản |
| GET | `/content/{source}/projects/{pid}` | Mô tả, ảnh, link, `server_side` |
| GET | `/content/{source}/projects/{pid}/versions?server_id=` | Bản hợp với server, mới nhất trước |
| GET | `/servers/{id}/content` | Danh sách mod hoặc plugin (mục 3.3), kèm `pending` |
| POST | `/servers/{id}/content` | `{ source, project_id, version_id? }` → Job |
| POST | `/servers/{id}/content/upload` | `multipart`, jar ≤ 512 MiB; khớp hash với `pending` thì xóa dòng chờ |
| PATCH | `/servers/{id}/content/{file}` | `{ enabled }` |
| DELETE | `/servers/{id}/content/{file}` | Chuyển vào thùng rác |
| GET | `/servers/{id}/preflight` | `{ missing_deps, wrong_loader, pending, client_only }` |
| POST | `/servers/{id}/updates/check` | Danh sách có bản mới → Job (gọi mạng nhiều) |
| POST | `/servers/{id}/updates` | `{ items }` hoặc `{ modpack_version_id }` → Job |
| POST | `/servers/{id}/version` | `{ minecraft?, loader_version?, confirm: true }` → Job |
| POST | `/servers/modpack` | `{ source, project_id, version_id, name, memory_mb?, port? }` → Job |
| POST | `/servers/modpack/upload` | `multipart` `.mrpack` hoặc `.zip` ≤ 512 MiB, kèm `name` → Job |
| GET / PUT / DELETE | `/settings/curseforge` | `{ available, source: "builtin" \| "user" \| "none" }` · `{ api_key }` |

- `POST /servers` nhận `loader` mới và `loader_version?`. `POST /servers/{id}/actions/start` nhận
  `ignore_preflight`. `PATCH /servers/{id}` nhận `java_major`.
- `ServerDto` thêm `loader_version`, `modpack`, `content_kind`, `preflight_blocking: bool`.
- WebSocket: Job đã có tiến độ; không thêm topic mới.

### 5.9 Binary `minehost`

- Đọc key nhúng `option_env!("MINEHOST_CURSEFORGE_KEY")` và truyền vào `mh-app`.
- Chế độ e2e thêm nguồn Modrinth và CurseForge giả (catalog cố định, file jar nhỏ dựng sẵn, một mod
  bị cấm phân phối) để Playwright chạy không cần mạng.

## 6. Các trang

Chỉ ghép từ component trong `ui/src/ui/`; component mới ở mục 6.6.

### 6.1 Tạo máy chủ `/new`

Bước 1 chọn kiểu: **Server thường** (Paper, Purpur, Vanilla như P2), **Server có mod** (Fabric,
NeoForge, Forge, Quilt, theo thứ tự này, mỗi loại một dòng giải thích ngắn), **Từ modpack** (sang
`/new/modpack`). Bước phiên bản chỉ hiện phiên bản loader có; chế độ Nâng cao thêm ô chọn bản loader.

### 6.2 Từ modpack `/new/modpack`

Ô tìm kiếm với nút chọn nguồn Modrinth / CurseForge (CurseForge mờ đi kèm lý do khi không có key),
lưới kết quả có ảnh, tên, số lượt tải, loader, phiên bản. Chọn pack → chọn bản (mặc định mới nhất) →
tên, RAM (đề xuất theo mục 5.7), cổng → Tạo. Vùng kéo thả nhận `.mrpack` hoặc `.zip`. Màn hình Job
hiện tiến độ từng file; xong thì hiện tóm tắt: số mod đã cài, số mod bỏ qua vì chỉ chạy client, file
CurseForge còn chờ (mục 6.4).

### 6.3 Mod hoặc Plugin `/servers/:id/content`

- Bảng: ảnh, tên, phiên bản, nguồn (Modrinth, CurseForge, Modpack, Thêm tay), công tắc bật/tắt, nhãn
  "Chỉ chạy ở client", nhãn "Có bản mới", menu "…" (Cập nhật, Gỡ, Mở trang mod). Ô lọc theo tên.
- Nút "Thêm" mở hộp thoại tìm kiếm đã lọc sẵn theo loader và phiên bản của server; chọn bản, bấm Cài,
  danh sách dependency sẽ cài kèm hiện trước khi xác nhận.
- Nút "Tải lên" nhận jar. Nút "Kiểm tra cập nhật".
- Server đang chạy thì công tắc và nút Gỡ bị khóa, kèm dòng giải thích.

### 6.4 File CurseForge chờ tải tay

Hộp ở đầu trang Mod khi `pending` không rỗng: mỗi dòng có tên mod, nút "Mở trang tải" (tab mới),
trạng thái. Một vùng kéo thả chung nhận nhiều file cùng lúc; file khớp hash thì dòng chuyển "Đã có",
sai thì báo `UPLOAD_HASH_MISMATCH` kèm tên file đúng. Có nút "Bật mà không có các mod này".

### 6.5 Cài đặt server và Cài đặt chung

- Cài đặt server, mục "Phiên bản": phiên bản Minecraft và loader hiện tại, nút "Đổi phiên bản",
  "Cập nhật modpack" (khi có), chọn Java (Nâng cao). Hộp thoại xác nhận nói rõ sẽ tắt server và tạo
  backup trước; hạ phiên bản thì cảnh báo đỏ (mục 3.9).
- Cài đặt chung, mục CurseForge: trạng thái key, ô nhập key riêng, nút Xóa. Mục "Bộ nhớ đệm tải về":
  dung lượng, nút Xóa. Mục "Giới hạn tài nguyên": đang giới hạn được hay không (mục 3.8).

### 6.6 Component mới

Chỉ thêm một component; phần còn lại dùng cái đã có: `SegmentedControl` (chọn nguồn), `ChoiceGroup`
(chọn kiểu server), `Switch` (bật/tắt mod), `Notice` với `tone` và `action` (băng chặn bật server),
`FileDrop`, `DataTable`, `Dialog`.

| Component | Dùng ở |
| --- | --- |
| `ProjectCard` (ảnh, tên, mô tả một dòng, số lượt tải, nhãn loader và phiên bản) | Tìm mod, tìm modpack |

## 7. Phụ thuộc mới

| Crate | Thư viện | Lý do |
| --- | --- | --- |
| `mh-providers`, `mh-store` | `zip` | Đọc jar và modpack |
| `mh-providers` | `quick-xml` | `maven-metadata.xml` của Forge, NeoForge |
| `mh-core` | `serde_yaml_ng` hoặc parser tay | `plugin.yml`; chọn khi viết PR, ưu tiên không thêm thư viện |
| `mh-net` | `sha1` | Hash của Modrinth và CurseForge (`sha2` đã có cho SHA-512) |

## 8. Bảo mật

- Mod và plugin là code tùy ý chạy với quyền của user. MineHost giới hạn RAM và số process (mục 3.8),
  nhưng không chặn mod đọc file. Trang Mod ghi rõ điều này cạnh nút "Tải lên" và khi cài từ nguồn
  "Thêm tay".
- Installer của loader là code của dự án loader, chạy với quyền của user và tự tải thư viện. MineHost
  chỉ tải installer từ maven chính thức qua HTTPS (mục 3.7).
- Modpack tải lên: đường dẫn trong index và overrides đi qua `SafeRoot`; URL qua danh sách host; bỏ
  symlink; giới hạn tổng kích thước giải nén 8 GiB và 20.000 file để chặn zip bomb.
- Key CurseForge không bao giờ ra khỏi controller (không trong API, log, backup).
- Upload jar và modpack qua cùng kiểm tra Origin, CSRF như upload của P2.

## 9. Test

### 9.1 Các lớp

| Lớp | Nội dung chính |
| --- | --- |
| Unit | Parser metadata jar (fixture thật của 20 mod phổ biến), `MrpackIndex`, `CfManifest`, ánh xạ NeoForge, nhận mod chỉ client, `memory_limit_mb` |
| Provider | Mỗi nguồn: fixture JSON và XML ghi sẵn, wiremock; 429 có `Retry-After`; host ngoài danh sách bị chặn |
| Store | `content.toml` giữ khóa lạ; `scan_content` ghép đúng khi file bị đổi tên, xóa tay, chuyển thư mục |
| App (`fake-mc`) | Installer giả thoát mã 1 → `LOADER_INSTALL_FAILED`; preflight chặn `start`; cập nhật lỗi giữa chừng vẫn còn bản `before_update`; vượt RAM → `memory_limit` |
| Runtime | Windows: Job Object có `JobMemoryLimit`; Linux: cgroup con có `memory.max`, `pids.max` (bỏ qua khi máy CI không có cgroup ghi được) |
| API | Mỗi route một test `oneshot`; key CurseForge không xuất hiện trong mọi response |
| Tích hợp (CI `integration`) | Mỗi loader một server thật tới `Done` trên Windows và Linux; một `.mrpack` nhỏ |
| Modpack (hằng đêm, runner tự host) | 20 pack ở mục 9.2 |
| E2E (Playwright) | Tạo server Fabric; cài mod kèm dependency; tắt mod; cài modpack từ nguồn giả; luồng file CurseForge chờ tải tay |

### 9.2 Danh sách 20 modpack

Chốt ở PR-37 trong file `tests/modpacks.toml` (pack, bản cố định, RAM, thời gian chờ `Done`), sau
khi thử từng pack trên runner. Tiêu chí chọn: đủ bốn loader (Fabric ≥ 6, NeoForge ≥ 5, Forge ≥ 5,
Quilt ≥ 1), cả Modrinth (≥ 10) và CurseForge (≥ 6), trải từ 1.12.2 tới bản Minecraft mới nhất có
pack, ít nhất 3 pack có mod chỉ client cần tắt, ít nhất 1 pack CurseForge có mod cấm phân phối (bước
tải tay dùng file đã cache sẵn trên runner), ít nhất 2 pack trên 200 mod. Mỗi pack cần tối đa 10 GB RAM.

Workflow ghi thời gian cài, thời gian tới `Done` và RSS lớn nhất vào artifact, để chỉnh công thức RAM
ở mục 3.8 và mục 5.7.

## 10. Rủi ro

| Rủi ro | Cách giảm |
| --- | --- |
| Installer Forge, NeoForge đổi tham số hoặc cấu trúc thư mục | Test tích hợp mỗi loader trên hai OS; đọc `args_file` thật sau khi cài, không đoán đường dẫn |
| CurseForge thu hồi key hoặc đổi điều khoản | Người dùng nhập key riêng được; Modrinth vẫn chạy độc lập |
| Phát hiện mod chỉ client sai, tắt nhầm mod cần cho server | Chỉ tắt khi có nguồn rõ (mục 3.5); hiện nhãn và cho bật lại; danh sách bỏ qua hiện trong tóm tắt Job |
| Giới hạn RAM kill nhầm server modpack lớn | Công thức rộng tay (gấp 1,5 lần heap cộng 1 GB); đo trong workflow modpack; lý do `memory_limit` hiện rõ |
| Modpack lớn tải lâu, mạng yếu | Cache theo hash dùng chung; tiếp tục từ file đã có khi chạy lại Job |
| Runner tự host làm chậm dịch vụ khác trên máy | Giới hạn của unit systemd (mục 3.11), mỗi lần một pack, chạy lúc 04:00 |
| Phạm vi P3 lớn | Chia thành hai bản phát hành (mục 11) |

## 11. Quyết định và câu hỏi khi duyệt

Đã chốt với chủ dự án ngày 2026-11-13:

1. CurseForge dùng key riêng của MineHost nhúng lúc build; người dùng ghi đè được (mục 3.6). Chủ dự
   án tự đăng ký key và đặt secret `MINEHOST_CURSEFORGE_KEY`.
2. Workflow modpack chạy trên runner tự host ở máy Linux thử nghiệm (mục 3.11).
3. Giới hạn RAM và số process vào đầu P3; chặn đọc file để backlog (mục 3.8).
4. PR-34 của P2 (chạy dài 72 giờ, `v0.3.0`) chạy song song với việc viết spec này.

Cần quyết định khi duyệt:

| Câu hỏi | Đề xuất |
| --- | --- |
| Chia P3 thành hai bản phát hành? | Có: `v0.4.0` sau nhóm A–D (bốn loader, Modrinth, `.mrpack`); `v0.5.0` sau nhóm E–H (CurseForge, cập nhật, workflow 20 pack). Người dùng có mod sớm hơn vài tuần |
| Plugin từ CurseForge (lớp Bukkit)? | Không trong P3; plugin chỉ từ Modrinth. Phần lớn plugin trên CurseForge cũng có trên Modrinth hoặc Hangar |
| Thứ tự loader trong wizard | Fabric, NeoForge, Forge, Quilt (theo số modpack trên Modrinth), Fabric đứng đầu vì nhẹ nhất |
| User-Agent ghi URL repo private | Giữ; Modrinth chỉ cần một cách liên hệ. Đổi khi repo public hoặc có trang web |
