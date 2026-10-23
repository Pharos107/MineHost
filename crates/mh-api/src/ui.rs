use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};
use rust_embed::RustEmbed;

/// Bản build của `ui/`. Bản release nhúng file vào binary; bản debug đọc từ đĩa.
#[derive(RustEmbed)]
#[folder = "../../ui/dist"]
#[allow_missing = true]
struct UiAssets;

const NOT_BUILT: &str = "<!doctype html><meta charset=\"utf-8\"><title>MineHost</title>\
<p>Giao diện chưa được build. Chạy <code>npm run build</code> trong thư mục <code>ui/</code> \
rồi build lại MineHost.</p>";

/// Trả file tĩnh nếu có; nếu không thì trả `index.html` để router phía client xử lý.
pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(response) = asset(path) {
        return response;
    }
    asset("index.html").unwrap_or_else(|| (StatusCode::OK, Html(NOT_BUILT)).into_response())
}

fn asset(path: &str) -> Option<Response> {
    let file = UiAssets::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    // File trong `assets/` có hash trong tên nên cache lâu được; `index.html` thì không.
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };

    let mut response = file.data.into_response();
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(mime.as_ref()) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    Some(response)
}
