//! Bảo vệ API trên localhost (spec P1, mục 5.8).
//!
//! Một trang web bất kỳ người dùng đang mở có thể gửi request tới `127.0.0.1:7878`. Vì vậy:
//! - `Host` phải là `127.0.0.1:<port>` hoặc `localhost:<port>`, chặn DNS rebinding.
//! - Request thay đổi dữ liệu phải có `Origin` của chính MineHost; thiếu `Origin` thì phải có
//!   `X-MineHost: 1` (cho CLI và test). Trình duyệt không cho trang khác tự đặt header này
//!   mà không qua preflight CORS, mà MineHost không trả lời CORS.
//! - Body của request thay đổi dữ liệu phải là JSON, để form HTML của trang khác không gửi được.
//!   Ngoại lệ: `POST /api/v1/servers/{id}/files/upload` và `.../content/upload` nhận `multipart/form-data`.
//! - WebSocket cũng phải đúng `Origin`, vì trình duyệt không áp dụng same-origin cho WebSocket.
//! - Từ P2: thêm `<ip-lan>:<cổng>` của các listener LAN đang mở (spec P2, mục 5.9).

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use mh_core::ErrorCode;

use crate::ApiError;
use crate::lan::LanState;

pub const MINEHOST_HEADER: &str = "x-minehost";

#[derive(Debug, Clone)]
pub struct LocalGuard {
    hosts: [String; 2],
    origins: [String; 2],
    lan: Arc<LanState>,
}

impl LocalGuard {
    pub fn new(port: u16) -> Self {
        Self::with_lan(port, LanState::new())
    }

    pub(crate) fn with_lan(port: u16, lan: Arc<LanState>) -> Self {
        Self {
            hosts: [format!("127.0.0.1:{port}"), format!("localhost:{port}")],
            origins: [
                format!("http://127.0.0.1:{port}"),
                format!("http://localhost:{port}"),
            ],
            lan,
        }
    }

    /// `Host` là localhost hoặc một listener LAN đang mở. Request đi vào listener nào thì
    /// do [`crate::lan::lan_listener`] đánh dấu theo kết nối, không dựa vào header này.
    fn host_ok(&self, req: &Request) -> bool {
        // HTTP/2 đặt host trong `:authority` (axum đưa vào URI) thay vì header `Host`.
        let host = req
            .headers()
            .get(header::HOST)
            .and_then(|h| h.to_str().ok())
            .map(str::to_owned)
            .or_else(|| req.uri().authority().map(|a| a.to_string()));
        host.is_some_and(|h| {
            let h = h.to_ascii_lowercase();
            self.hosts.contains(&h) || self.lan.has_host(&h)
        })
    }

    /// Có `Origin` thì phải đúng; không có thì phải có `X-MineHost: 1`.
    pub(crate) fn origin_ok(&self, headers: &HeaderMap) -> bool {
        match headers.get(header::ORIGIN).map(|o| o.to_str()) {
            Some(Ok(origin)) => self.origin_known(&origin.to_ascii_lowercase()),
            Some(Err(_)) => false,
            None => headers
                .get(MINEHOST_HEADER)
                .is_some_and(|v| v.as_bytes() == b"1"),
        }
    }

    fn origin_known(&self, origin: &str) -> bool {
        if self.origins.iter().any(|known| known == origin) {
            return true;
        }
        origin
            .strip_prefix("http://")
            .is_some_and(|host| self.lan.has_host(host))
    }
}

pub async fn check(State(guard): State<LocalGuard>, req: Request, next: Next) -> Response {
    if !guard.host_ok(&req) {
        tracing::warn!(host = ?req.headers().get(header::HOST), "chặn request có Host lạ");
        return forbidden("host");
    }
    let mutating = is_mutating(req.method());
    // WebSocket là GET nhưng mở kênh hai chiều (gửi được lệnh console), nên cũng phải đúng Origin.
    if (mutating || is_websocket(req.headers())) && !guard.origin_ok(req.headers()) {
        tracing::warn!(origin = ?req.headers().get(header::ORIGIN), "chặn request có Origin lạ");
        return forbidden("origin");
    }
    // Multipart chỉ đúng route tải file. Route khác vẫn bắt JSON để form của trang lạ không gửi được.
    if mutating && has_body(req.headers()) && !is_json(req.headers()) && !is_upload(&req) {
        return ApiError::new(StatusCode::UNSUPPORTED_MEDIA_TYPE, ErrorCode::InvalidInput)
            .with_params(serde_json::json!({ "field": "content-type" }))
            .into_response();
    }
    next.run(req).await
}

pub(crate) fn forbidden(reason: &str) -> Response {
    ApiError::new(StatusCode::FORBIDDEN, ErrorCode::Forbidden)
        .with_params(serde_json::json!({ "reason": reason }))
        .into_response()
}

pub(crate) fn is_mutating(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn is_websocket(headers: &HeaderMap) -> bool {
    headers
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"))
}

fn has_body(headers: &HeaderMap) -> bool {
    let length = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    length.is_some_and(|n| n > 0) || headers.contains_key(header::TRANSFER_ENCODING)
}

fn is_json(headers: &HeaderMap) -> bool {
    mime_is(headers, "application/json")
}

fn is_upload(req: &Request) -> bool {
    req.method() == Method::POST
        && is_upload_path(req.uri().path())
        && mime_is(req.headers(), "multipart/form-data")
}

/// Đúng path upload, không phải tiền tố. Guard chạy ngoài `nest`, nên path còn `/api/v1`.
fn is_upload_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/api/v1/servers/") else {
        return false;
    };
    if rest == "modpack/upload" {
        return true;
    }
    let Some((_, tail)) = rest.split_once('/') else {
        return false;
    };
    matches!(tail, "files/upload" | "content/upload")
}

fn mime_is(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case(expected))
}

#[cfg(test)]
mod tests {
    use super::is_upload_path;

    #[test]
    fn only_exact_upload_path_is_multipart() {
        let id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        assert!(is_upload_path(&format!(
            "/api/v1/servers/{id}/files/upload"
        )));
        assert!(is_upload_path(&format!(
            "/api/v1/servers/{id}/content/upload"
        )));
        assert!(!is_upload_path(&format!(
            "/api/v1/servers/{id}/files/mkdir"
        )));
        assert!(!is_upload_path(&format!(
            "/api/v1/servers/{id}/files/upload/extra"
        )));
        assert!(!is_upload_path(&format!(
            "/api/v1/servers/{id}/content/upload/extra"
        )));
        assert!(is_upload_path("/api/v1/servers/modpack/upload"));
        assert!(!is_upload_path("/api/v1/servers/modpack/upload/extra"));
        assert!(!is_upload_path("/api/v1/servers/files/upload"));
    }
}
