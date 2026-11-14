# CurseForge REST API v1 fixtures

**Lưu ý:** Các file JSON trong thư mục này **không phải response thật** từ API CurseForge (máy phát triển không có API key CurseForge để gọi trực tiếp). Chúng được **dựng lại theo đúng cấu trúc và tên trường** trong ví dụ response của tài liệu API chính thức CurseForge, điền giá trị minh họa hợp lý cho mod/file/modpack để test `wiremock`.

Tài liệu tham khảo chính thức:

- Tổng quan REST API: https://docs.curseforge.com/rest-api/
- Search Mods (`GET /v1/mods/search`): https://docs.curseforge.com/rest-api/#search-mods
- Get Mod (`GET /v1/mods/{modId}`): https://docs.curseforge.com/rest-api/#get-mod
- Get Mod Files (`GET /v1/mods/{modId}/files`): https://docs.curseforge.com/rest-api/#get-mod-files
- Get Files (`POST /v1/mods/files`): https://docs.curseforge.com/rest-api/#get-files

| File | Endpoint tương ứng | Mục đích test |
| --- | --- | --- |
| `search_mods.json` | `GET /v1/mods/search` | Kết quả tìm kiếm với `data` + `pagination`, `downloadCount` kiểu số thực |
| `mod_238222.json` | `GET /v1/mods/{modId}` | Chi tiết mod có `allowModDistribution: true`, `logo`, `links` |
| `mod_restricted.json` | `GET /v1/mods/{modId}` | Mod có `allowModDistribution: false` (cấm phân phối qua API) |
| `mod_files.json` | `GET /v1/mods/{modId}/files` | Danh sách file chưa sắp thứ tự để kiểm tra sắp xếp mới nhất trước, SHA-1 (`algo = 1`), dependency (`relationType = 3`) |
| `files_by_id.json` | `POST /v1/mods/files` | Batch file gồm cả file hợp lệ và file có `downloadUrl: null` |
