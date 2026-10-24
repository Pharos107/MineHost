//! REST API và WebSocket của MineHost tại `/api/v1`, kèm UI tĩnh ở mọi đường dẫn còn lại.

mod dto;
mod error;
mod guard;
mod jobs;
mod servers;
mod system;
mod ui;
mod ws;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use mh_app::App;
use utoipa::OpenApi;

pub use error::ApiError;
pub use guard::{LocalGuard, MINEHOST_HEADER};

#[derive(OpenApi)]
#[openapi(
    info(title = "MineHost API", description = "API của controller MineHost."),
    paths(
        system::health,
        system::system_info,
        servers::list,
        servers::create,
        servers::get,
        servers::patch,
        servers::trash,
        servers::eula,
        servers::action,
        servers::send_command,
        servers::console,
        servers::get_properties,
        servers::put_properties,
        jobs::get,
        jobs::cancel,
        jobs::paper_versions,
        jobs::shutdown,
    ),
    components(schemas(
        error::ErrorBody,
        system::Health,
        system::SystemDto,
        system::DiskDto,
        dto::ServerDto,
        dto::StateDto,
        dto::MemoryDto,
        dto::CrashDto,
        dto::CreateServerBody,
        dto::CreatedDto,
        dto::PatchServerBody,
        dto::EulaBody,
        dto::TrashedDto,
        dto::CommandBody,
        dto::ConsoleDto,
        dto::ConsoleLineDto,
        dto::StreamDto,
        dto::PropertiesDto,
        dto::PropertyDto,
        dto::CatalogVersionDto,
        dto::JobDto,
        dto::JobStateDto,
        dto::ProgressDto,
    ))
)]
struct ApiDoc;

/// Spec OpenAPI, dùng để sinh client TypeScript cho `ui/`.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

/// `port` là cổng controller thật sự đang nghe, dùng để kiểm tra `Host` và `Origin`.
pub fn router(app: Arc<App>, port: u16) -> Router {
    let api = Router::new()
        .route("/health", get(system::health))
        .route("/system", get(system::system_info))
        .route("/servers", get(servers::list).post(servers::create))
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
        .route(
            "/servers/{id}/properties",
            get(servers::get_properties).put(servers::put_properties),
        )
        .route("/jobs/{id}", get(jobs::get))
        .route("/jobs/{id}/cancel", post(jobs::cancel))
        .route("/catalog/paper/versions", get(jobs::paper_versions))
        .route("/app/shutdown", post(jobs::shutdown))
        .route("/ws", get(ws::upgrade))
        .fallback(error::api_not_found)
        .with_state(app);

    Router::new()
        .nest("/api/v1", api)
        .fallback(ui::serve)
        .layer(axum::middleware::from_fn_with_state(
            LocalGuard::new(port),
            guard::check,
        ))
}

#[cfg(test)]
mod tests;
