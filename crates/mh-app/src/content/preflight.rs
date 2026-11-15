//! Kiểm tra điều kiện mod/plugin trước khi bật server (spec P3, mục 3.5 & 4.4).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use mh_core::{
    ErrorCode, FabricDepOverrides, InstanceSpec, LoaderKind, ModMeta, ModMetaFormat, ServerId,
    matches_version_req, parse_fabric_dep_overrides,
};
use mh_store::{ContentFile, ContentManifest, PendingContent};
use serde_json::json;

use crate::files::blocking;
use crate::{App, AppError};

/// Báo cáo kiểm tra mod/plugin trước khi bật server.
///
/// Một dependency coi là đã có khi trùng `id`, một mục `provides`, hoặc một mod trong jar lồng của
/// mod đang bật, giống cách loader giải quyết. Jar đa loader được đối chiếu bằng metadata mà loader
/// của server đọc (Fabric chỉ đọc `fabric.mod.json` dù jar có cả `quilt.mod.json`). Server Fabric áp
/// thêm `config/fabric_loader_dependencies.json` như loader. Các id nền tảng
/// (`minecraft`, `java`, `fabricloader`, `forge`, `neoforge`, `quilt_loader`, `fml`) đã được lọc bỏ
/// tại `mh_core::mod_meta`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreflightReport {
    pub missing_deps: Vec<MissingDependency>,
    /// Có mod cùng `id` nhưng phiên bản hiện tại không khớp ràng buộc (chỉ cảnh báo, không chặn).
    pub version_mismatches: Vec<VersionMismatch>,
    pub wrong_loader: Vec<WrongLoaderMod>,
    pub pending: Vec<PendingContent>,
    pub client_only: Vec<ClientOnlyMod>,
}

impl PreflightReport {
    /// Chỉ chặn bật khi còn thiếu dependency bắt buộc hoặc còn file CurseForge đang chờ tải tay.
    pub fn is_blocking(&self) -> bool {
        !self.missing_deps.is_empty() || !self.pending.is_empty()
    }
}

/// Một dependency bắt buộc mà chưa tìm thấy trong các mod đang bật.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingDependency {
    pub id: String,
    pub version_req: String,
    pub required_by: String,
}

/// Dependency có mặt nhưng phiên bản không thỏa ràng buộc (không chặn bật).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionMismatch {
    pub id: String,
    pub version_req: String,
    pub installed_version: String,
    pub required_by: String,
}

/// Mod/plugin đang bật nhưng jar chỉ có metadata của loader khác với server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrongLoaderMod {
    pub file_name: String,
    pub name: String,
    pub formats: Vec<ModMetaFormat>,
}

/// Mod đang bật khai báo chỉ chạy phía client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientOnlyMod {
    pub file_name: String,
    pub name: String,
}

impl App {
    /// Kiểm tra mod/plugin trước khi bật server (dependency, sai loader, pending CurseForge, chỉ client).
    ///
    /// Server Vanilla không dùng mod/plugin nên luôn trả báo cáo rỗng. Các id nền tảng và id ảo
    /// đã lọc sẵn ở `mh_core::mod_meta` không bị báo thiếu; xem thêm doc comment ở `PreflightReport`.
    pub async fn preflight(&self, id: ServerId) -> Result<PreflightReport, AppError> {
        let spec = self.store.load(id)?;
        let Some(kind) = spec.game.loader.content_kind() else {
            return Ok(PreflightReport::default());
        };
        let mutex = self.content_lock(id);
        let _guard = mutex.lock().await;
        let (store, scanner) = (self.store.clone(), Arc::clone(&self.content_scanner));
        let report = blocking(move || {
            let paths = store.paths(id);
            let files = scanner.scan_dirs(&paths.server_dir, &paths.content_file, kind)?;
            let manifest = ContentManifest::load(&paths.content_file)?;
            let overrides = load_dep_overrides(spec.game.loader, &paths.server_dir, id);
            Ok(build_report(
                spec.game.loader,
                &files,
                &overrides,
                manifest.pending,
            ))
        })
        .await?;
        crate::lock(&self.preflight_missing).insert(id, !report.missing_deps.is_empty());
        Ok(report)
    }

    /// Chạy lại `preflight` sau khi mod thay đổi để cờ trong `ServerView` không cũ. Lỗi chỉ ghi log:
    /// thao tác chính đã xong, cờ sai tạm thời không đáng làm hỏng nó.
    pub(crate) async fn refresh_preflight(&self, id: ServerId) {
        if let Err(err) = self.preflight(id).await {
            tracing::warn!(server = %id, %err, "không cập nhật được preflight sau khi đổi mod");
        }
    }

    /// Điền `ServerView.preflight_blocking` mà không quét jar: hàm này chạy đồng bộ mỗi lần lấy
    /// danh sách server, quét (và tính SHA-1) hàng trăm jar ở đây sẽ chặn luồng async. File
    /// CurseForge chờ tải chỉ cần đọc `content.toml`; phần thiếu dependency lấy từ lần `preflight`
    /// gần nhất (trang server gọi nó khi mở và sau mỗi thay đổi), chưa chạy lần nào thì coi là không chặn.
    pub(crate) fn has_preflight_blocking(&self, spec: &InstanceSpec) -> bool {
        if spec.game.loader.content_kind().is_none() {
            return false;
        }
        let paths = self.store.paths(spec.id);
        let pending = match ContentManifest::load(&paths.content_file) {
            Ok(m) => !m.pending.is_empty(),
            Err(err) => {
                tracing::warn!(server = %spec.id, %err, "không đọc được content.toml khi kiểm tra preflight");
                false
            }
        };
        pending
            || crate::lock(&self.preflight_missing)
                .get(&spec.id)
                .copied()
                .unwrap_or(false)
    }
}

pub(crate) fn preflight_block_error(report: &PreflightReport) -> Option<AppError> {
    if !report.pending.is_empty() {
        let files: Vec<&str> = report.pending.iter().map(|p| p.path.as_str()).collect();
        return Some(
            AppError::new(
                ErrorCode::CurseforgeFilesPending,
                "còn file CurseForge cần tải tay trước khi bật server",
            )
            .with_params(json!({ "pending": files })),
        );
    }
    if !report.missing_deps.is_empty() {
        let missing: Vec<_> = report
            .missing_deps
            .iter()
            .map(|d| {
                json!({
                    "id": d.id,
                    "version_req": d.version_req,
                    "required_by": d.required_by,
                })
            })
            .collect();
        return Some(
            AppError::new(
                ErrorCode::DependencyMissing,
                "còn thiếu dependency bắt buộc trước khi bật server",
            )
            .with_params(json!({ "missing": missing })),
        );
    }
    None
}

/// Đường dẫn file ghi đè dependency của Fabric loader, tính từ thư mục `server/`.
const FABRIC_DEP_OVERRIDES: &str = "config/fabric_loader_dependencies.json";

/// Đọc ghi đè dependency mà Fabric loader sẽ áp dụng. Thiếu file là bình thường; file hỏng thì
/// Fabric cũng từ chối khởi động, nên chỉ ghi log và kiểm tra như không có ghi đè.
fn load_dep_overrides(loader: LoaderKind, server_dir: &Path, id: ServerId) -> FabricDepOverrides {
    if loader != LoaderKind::Fabric {
        return FabricDepOverrides::default();
    }
    let path = server_dir.join(FABRIC_DEP_OVERRIDES);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            if err.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(server = %id, %err, "không đọc được {FABRIC_DEP_OVERRIDES}");
            }
            return FabricDepOverrides::default();
        }
    };
    parse_fabric_dep_overrides(&text).unwrap_or_else(|err| {
        tracing::warn!(server = %id, %err, "{FABRIC_DEP_OVERRIDES} không hợp lệ, bỏ qua");
        FabricDepOverrides::default()
    })
}

fn build_report(
    loader: LoaderKind,
    files: &[ContentFile],
    overrides: &FabricDepOverrides,
    pending: Vec<PendingContent>,
) -> PreflightReport {
    let (missing_deps, version_mismatches) = check_dependencies(loader, files, overrides);
    let mut wrong_loader = Vec::new();
    let mut client_only = Vec::new();

    for file in files.iter().filter(|f| f.enabled) {
        let name = display_name(file);
        if !file.formats.is_empty() && !supports_any_format(loader, &file.formats) {
            wrong_loader.push(WrongLoaderMod {
                file_name: file.file_name.clone(),
                name: name.clone(),
                formats: file.formats.clone(),
            });
        }
        let is_client = file.meta.as_ref().is_ok_and(|m| m.side.is_client_only())
            || file.record.as_ref().is_some_and(|r| r.client_only);
        if is_client {
            client_only.push(ClientOnlyMod {
                file_name: file.file_name.clone(),
                name,
            });
        }
    }

    PreflightReport {
        missing_deps,
        version_mismatches,
        wrong_loader,
        pending,
        client_only,
    }
}

fn check_dependencies(
    loader: LoaderKind,
    files: &[ContentFile],
    overrides: &FabricDepOverrides,
) -> (Vec<MissingDependency>, Vec<VersionMismatch>) {
    let enabled: Vec<&ContentFile> = files.iter().filter(|f| f.enabled).collect();
    let metas: Vec<&ModMeta> = enabled
        .iter()
        .filter_map(|f| loader_meta(loader, f))
        .collect();
    let mut by_id: HashMap<String, Vec<&ModMeta>> = HashMap::new();
    let bundled = enabled.iter().flat_map(|f| f.bundled.iter());
    for meta in metas.iter().copied().chain(bundled) {
        for id in std::iter::once(&meta.id).chain(&meta.provides) {
            by_id.entry(id.to_ascii_lowercase()).or_default().push(meta);
        }
    }

    let mut missing = Vec::new();
    let mut mismatches = Vec::new();
    for meta in metas {
        let depends = overrides.effective_depends(meta);
        for dep in depends.iter().filter(|d| d.required) {
            match by_id.get(&dep.id.to_ascii_lowercase()) {
                None => missing.push(MissingDependency {
                    id: dep.id.clone(),
                    version_req: dep.version_req.clone(),
                    required_by: meta.name.clone(),
                }),
                Some(candidates) => {
                    if let Some(mismatch) = check_version_match(&meta.name, dep, candidates) {
                        mismatches.push(mismatch);
                    }
                }
            }
        }
    }
    (missing, mismatches)
}

/// Metadata mà loader của server sẽ đọc; jar không có định dạng đó thì dùng bản chính.
fn loader_meta(loader: LoaderKind, file: &ContentFile) -> Option<&ModMeta> {
    use ModMetaFormat as F;
    let preferred: &[ModMetaFormat] = match loader {
        LoaderKind::Fabric => &[F::FabricModJson],
        LoaderKind::Quilt => &[F::QuiltModJson, F::FabricModJson],
        LoaderKind::Forge => &[F::ModsToml],
        LoaderKind::NeoForge => &[F::NeoForgeModsToml, F::ModsToml],
        LoaderKind::Paper | LoaderKind::Purpur => &[F::PaperPluginYml, F::PluginYml],
        LoaderKind::Vanilla | _ => &[],
    };
    preferred
        .iter()
        .find_map(|want| {
            file.format_metas
                .iter()
                .find(|(fmt, _)| fmt == want)
                .map(|(_, meta)| meta)
        })
        .or_else(|| file.meta.as_ref().ok())
}

fn check_version_match(
    required_by: &str,
    dep: &mh_core::ModDependency,
    candidates: &[&ModMeta],
) -> Option<VersionMismatch> {
    let mut first_mismatch: Option<&str> = None;
    for candidate in candidates {
        let Some(ver) = candidate.version.as_deref() else {
            // Thiếu số phiên bản trong jar thì không kết luận sai bản.
            return None;
        };
        match matches_version_req(ver, &dep.version_req) {
            Some(true) | None => return None,
            Some(false) => {
                first_mismatch.get_or_insert(ver);
            }
        }
    }
    first_mismatch.map(|ver| VersionMismatch {
        id: dep.id.clone(),
        version_req: dep.version_req.clone(),
        installed_version: ver.to_owned(),
        required_by: required_by.to_owned(),
    })
}

fn supports_any_format(loader: LoaderKind, formats: &[ModMetaFormat]) -> bool {
    formats.iter().any(|fmt| match loader {
        LoaderKind::Fabric => *fmt == ModMetaFormat::FabricModJson,
        // Quilt chạy được cả mod Fabric lẫn mod Quilt.
        LoaderKind::Quilt => matches!(
            fmt,
            ModMetaFormat::QuiltModJson | ModMetaFormat::FabricModJson
        ),
        LoaderKind::Forge => *fmt == ModMetaFormat::ModsToml,
        LoaderKind::NeoForge => *fmt == ModMetaFormat::NeoForgeModsToml,
        LoaderKind::Paper | LoaderKind::Purpur => {
            matches!(
                fmt,
                ModMetaFormat::PaperPluginYml | ModMetaFormat::PluginYml
            )
        }
        LoaderKind::Vanilla | _ => true,
    })
}

fn display_name(file: &ContentFile) -> String {
    file.meta
        .as_ref()
        .map(|m| m.name.clone())
        .unwrap_or_else(|_| file.file_name.clone())
}
