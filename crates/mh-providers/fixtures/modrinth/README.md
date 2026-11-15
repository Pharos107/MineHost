# Modrinth API v2 fixtures

Lấy trực tiếp từ Modrinth API v2 ngày 2026-11-14 (rút gọn bớt phần tử thừa trong mảng, giữ nguyên giá trị).

| File | Nguồn |
| --- | --- |
| `search_sodium.json` | `GET https://api.modrinth.com/v2/search?query=sodium&limit=5` (giữ 3 hit: `AANobbMI`, `PtjYWJkn`, `EIa1eiMm`) |
| `project_sodium.json` | `GET https://api.modrinth.com/v2/project/AANobbMI` (rút gọn mảng `versions` và `gallery`) |
| `versions_sodium_fabric_1_21_1.json` | `GET https://api.modrinth.com/v2/project/AANobbMI/version?loaders=["fabric"]&game_versions=["1.21.1"]` (giữ 3 bản đầu) |
| `version_SMxNOGZ6.json` | `GET https://api.modrinth.com/v2/version/SMxNOGZ6` |
| `version_bAo1Qhte_iris.json` | `GET https://api.modrinth.com/v2/version/bAo1Qhte` (có dependency `required` tới Sodium `AANobbMI`) |
| `version_files.json` | `POST https://api.modrinth.com/v2/version_files` với `{"hashes":["003c114c85ca88ef3362e018deb6aca0c682d6a1","d01f2112698d08b052d042c68c1206af790e660a","0000000000000000000000000000000000000000"],"algorithm":"sha1"}` |
