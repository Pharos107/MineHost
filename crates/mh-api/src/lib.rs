//! REST API và WebSocket của MineHost tại `/api/v1`, kèm UI tĩnh ở mọi đường dẫn còn lại.

mod access;
mod auth;
mod backups;
mod dto;
mod error;
mod files;
mod guard;
mod import;
mod jobs;
mod lan;
mod servers;
mod system;
mod ui;
mod ws;

use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use mh_app::App;
use utoipa::OpenApi;

pub use error::ApiError;
pub use guard::{LocalGuard, MINEHOST_HEADER};
pub use lan::{LanState, lan_listener};

#[derive(OpenApi)]
#[openapi(
    info(title = "MineHost API", description = "API của controller MineHost."),
    paths(
        system::health,
        system::system_info,
        servers::list,
        servers::create,
        import::inspect,
        import::import,
        servers::get,
        servers::patch,
        servers::trash,
        servers::eula,
        servers::action,
        servers::send_command,
        servers::console,
        servers::metrics,
        backups::list,
        backups::create,
        backups::patch,
        backups::delete,
        backups::download,
        backups::restore,
        servers::get_properties,
        servers::put_properties,
        jobs::list,
        jobs::get,
        jobs::cancel,
        jobs::catalog_versions,
        jobs::shutdown,
        auth::session,
        auth::login,
        auth::logout,
        auth::put_password,
        auth::delete_password,
        access::get_access,
        access::put_access,
        files::list,
        files::delete_file,
        files::read,
        files::write,
        files::download,
        files::upload,
        files::mkdir,
        files::rename_file,
    ),
    components(schemas(
        error::ErrorBody,
        auth::SessionDto,
        auth::CsrfDto,
        auth::LoginBody,
        auth::PasswordBody,
        auth::ClearPasswordBody,
        access::AccessDto,
        access::AccessBody,
        system::Health,
        system::SystemDto,
        system::DiskDto,
        dto::ServerDto,
        dto::StateDto,
        dto::MemoryDto,
        dto::CrashDto,
        dto::CreateServerBody,
        dto::CreatedDto,
        dto::BackupSettingsDto,
        dto::BackupSettingsBody,
        backups::BackupDto,
        backups::CreateBackupBody,
        backups::PatchBackupBody,
        backups::RestoreBody,
        backups::JobStartedDto,
        import::InspectBody,
        import::DetectedDto,
        import::ImportBody,
        dto::PatchServerBody,
        dto::EulaBody,
        dto::TrashedDto,
        dto::CommandBody,
        dto::ConsoleDto,
        dto::ConsoleLineDto,
        dto::ServerMetricsDto,
        dto::MetricSampleDto,
        dto::StreamDto,
        dto::PropertiesDto,
        dto::PropertyDto,
        dto::CatalogVersionDto,
        dto::JobDto,
        dto::JobStateDto,
        dto::ProgressDto,
        files::FileEntryDto,
        files::FileKindDto,
        files::TextFileDto,
        files::TextBody,
        files::EtagDto,
        files::MkdirBody,
        files::RenameBody,
    ))
)]
struct ApiDoc;

/// Spec OpenAPI, dùng để sinh client TypeScript cho `ui/`.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

/// `port` là cổng controller thật sự đang nghe, dùng để kiểm tra `Host` và `Origin`.
pub fn router(app: Arc<App>, port: u16) -> Router {
    router_with_lan(app, port, LanState::new())
}

/// Như [`router`], kèm danh sách listener LAN mà binary cập nhật. Binary phục vụ cùng một
/// router (bản clone) trên `127.0.0.1` và trên từng địa chỉ LAN, nên phiên đăng nhập dùng chung.
pub fn router_with_lan(app: Arc<App>, port: u16, lan: Arc<LanState>) -> Router {
    router_with(app, port, files::UPLOAD_MAX_BYTES, lan)
}

/// Như [`router`], nhưng test được đặt giới hạn từng file upload nhỏ hơn 512 MiB.
pub(crate) fn router_with(
    app: Arc<App>,
    port: u16,
    upload_max_bytes: u64,
    lan: Arc<LanState>,
) -> Router {
    let guard = LocalGuard::with_lan(port, Arc::clone(&lan));
    let api = api_routes(auth::ApiState::new(app, lan), upload_max_bytes);
    // Guard thêm sau nên chạy trước: Host/Origin của P1 vẫn có hiệu lực.
    Router::new()
        .nest("/api/v1", api)
        .fallback(ui::serve)
        .layer(axum::middleware::from_fn_with_state(guard, guard::check))
}

/// Phiên bọc route API, không bọc file tĩnh: trang đăng nhập phải tải được khi chưa có cookie.
fn api_routes(state: auth::ApiState, upload_max_bytes: u64) -> Router {
    auth::routes()
        .route("/health", get(system::health))
        .route("/system", get(system::system_info))
        .route("/servers", get(servers::list).post(servers::create))
        .route("/servers/import", post(import::import))
        .route("/servers/import/inspect", post(import::inspect))
        .route(
            "/servers/{id}",
            get(servers::get)
                .patch(servers::patch)
                .delete(servers::trash),
        )
        .route("/servers/{id}/eula", post(servers::eula))
        .route("/servers/{id}/actions/{action}", post(servers::action))
        .route(
            "/servers/{id}/console",
            get(servers::console).post(servers::send_command),
        )
        .route("/servers/{id}/metrics", get(servers::metrics))
        .route(
            "/servers/{id}/backups",
            get(backups::list).post(backups::create),
        )
        .route(
            "/servers/{id}/backups/{bid}",
            axum::routing::patch(backups::patch).delete(backups::delete),
        )
        .route(
            "/servers/{id}/backups/{bid}/download",
            get(backups::download),
        )
        .route(
            "/servers/{id}/backups/{bid}/restore",
            post(backups::restore),
        )
        .route(
            "/servers/{id}/properties",
            get(servers::get_properties).put(servers::put_properties),
        )
        .route("/jobs", get(jobs::list))
        .route(
            "/servers/{id}/files",
            get(files::list).delete(files::delete_file),
        )
        .route(
            "/servers/{id}/files/content",
            get(files::read).put(files::write),
        )
        .route("/servers/{id}/files/download", get(files::download))
        .route("/servers/{id}/files/mkdir", post(files::mkdir))
        .route("/servers/{id}/files/rename", post(files::rename_file))
        // Chỉ route này nới giới hạn body. Route khác giữ mặc định 2 MiB của axum.
        .route(
            "/servers/{id}/files/upload",
            post(files::upload)
                .layer(DefaultBodyLimit::max(files::upload_body_limit(
                    upload_max_bytes,
                )))
                .layer(axum::middleware::from_fn_with_state(
                    upload_max_bytes,
                    files::inject_upload_limit,
                )),
        )
        .route("/jobs/{id}", get(jobs::get))
        .route("/jobs/{id}/cancel", post(jobs::cancel))
        .route("/catalog/{loader}/versions", get(jobs::catalog_versions))
        .route("/app/shutdown", post(jobs::shutdown))
        .route(
            "/settings/access",
            get(access::get_access).put(access::put_access),
        )
        .route("/ws", get(ws::upgrade))
        .fallback(error::api_not_found)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_session,
        ))
        .with_state(state)
}

#[cfg(test)]
mod access_tests;
#[cfg(test)]
mod auth_tests;
#[cfg(test)]
mod tests;
