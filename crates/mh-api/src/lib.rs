//! REST API của MineHost tại `/api/v1`, kèm UI tĩnh ở mọi đường dẫn còn lại.

mod error;
mod system;
mod ui;

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use mh_app::App;
use utoipa::OpenApi;

pub use error::ApiError;

#[derive(OpenApi)]
#[openapi(
    info(title = "MineHost API", description = "API của controller MineHost."),
    paths(system::health, system::system_info),
    components(schemas(error::ErrorBody, system::Health, system::SystemDto, system::DiskDto))
)]
struct ApiDoc;

/// Spec OpenAPI, dùng để sinh client TypeScript cho `ui/`.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

pub fn router(app: Arc<App>) -> Router {
    let api = Router::new()
        .route("/health", get(system::health))
        .route("/system", get(system::system_info))
        .fallback(error::api_not_found)
        .with_state(app);

    Router::new().nest("/api/v1", api).fallback(ui::serve)
}

#[cfg(test)]
mod tests;
