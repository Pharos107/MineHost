//! Phiên đăng nhập, CSRF và giới hạn đăng nhập sai (spec P2, mục 3.3, 5.8, 6.10, 8).
//!
//! Phiên chỉ nằm trong RAM: tắt MineHost là mất. Cookie không đặt `Secure` vì giao diện đi qua HTTP.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{ConnectInfo, FromRef, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use mh_app::{App, AppError};
use mh_core::ErrorCode;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use utoipa::ToSchema;

use crate::auth_gate::{AuthGate, SESSION_DAYS};
use crate::error::{ApiError, ApiJson, ErrorBody};
use crate::guard;
use crate::lan::{LanState, ViaLan};

pub(crate) const SESSION_COOKIE: &str = "mh_session";
const CSRF_HEADER: HeaderName = HeaderName::from_static("x-csrf-token");

#[derive(Clone)]
pub(crate) struct ApiState {
    app: Arc<App>,
    auth: Arc<AuthGate>,
    lan: Arc<LanState>,
}

impl ApiState {
    pub(crate) fn new(app: Arc<App>, lan: Arc<LanState>) -> Self {
        Self {
            app,
            auth: Arc::new(AuthGate::default()),
            lan,
        }
    }
}

impl FromRef<ApiState> for Arc<App> {
    fn from_ref(state: &ApiState) -> Self {
        Arc::clone(&state.app)
    }
}

impl FromRef<ApiState> for Arc<LanState> {
    fn from_ref(state: &ApiState) -> Self {
        Arc::clone(&state.lan)
    }
}

// Không `Debug`: mật khẩu và CSRF có thể đi vào log.
#[derive(Serialize, ToSchema)]
#[allow(missing_debug_implementations)]
pub struct SessionDto {
    pub password_set: bool,
    pub logged_in: bool,
    /// Có khi đang đăng nhập. Gửi lại bằng header `X-CSRF-Token`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csrf_token: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[allow(missing_debug_implementations)]
pub struct CsrfDto {
    pub csrf_token: String,
}

#[derive(Deserialize, ToSchema)]
#[allow(missing_debug_implementations)]
pub struct LoginBody {
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
#[allow(missing_debug_implementations)]
pub struct PasswordBody {
    pub current: Option<String>,
    pub new: String,
}

#[derive(Deserialize, ToSchema)]
#[allow(missing_debug_implementations)]
pub struct ClearPasswordBody {
    pub current: String,
}

pub(crate) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/auth/session", get(session))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/password", put(put_password).delete(delete_password))
}

/// Chưa đặt mật khẩu thì để nguyên như P1. Đã đặt thì mọi route trừ session và login cần cookie,
/// kể cả từ `127.0.0.1` (ADR 0003). File tĩnh không đi qua đây.
pub async fn require_session(State(state): State<ApiState>, req: Request, next: Next) -> Response {
    // Đọc mật khẩu và cờ LAN một lần, để hai lần đọc không thấy hai trạng thái khác nhau.
    let (password_set, lan_on) = match state.app.auth().snapshot() {
        Ok(snapshot) => snapshot,
        Err(err) => return ApiError::from(err).into_response(),
    };
    // Listener LAN còn mở thêm vài giây sau `reset-password` hoặc khi tắt LAN. Trong lúc đó
    // request từ LAN bị chặn ở đây, không bao giờ đi nhánh "chưa có mật khẩu" của P1.
    if req.extensions().get::<ViaLan>().is_some() && !lan_on {
        return ApiError::new(StatusCode::FORBIDDEN, ErrorCode::Forbidden)
            .with_params(serde_json::json!({ "reason": "lan" }))
            .into_response();
    }
    if is_public(req.uri().path()) || !password_set {
        return next.run(req).await;
    }
    let Some(csrf) = session_csrf(&state.auth, req.headers()) else {
        return ApiError::new(StatusCode::UNAUTHORIZED, ErrorCode::AuthRequired).into_response();
    };
    if guard::is_mutating(req.method()) && !csrf_matches(&csrf, req.headers()) {
        return ApiError::new(StatusCode::FORBIDDEN, ErrorCode::Forbidden)
            .with_params(serde_json::json!({ "reason": "csrf" }))
            .into_response();
    }
    next.run(req).await
}

#[utoipa::path(get, path = "/api/v1/auth/session", tag = "auth",
    responses((status = 200, body = SessionDto), (status = 422, body = ErrorBody)))]
pub async fn session(
    State(state): State<ApiState>,
    jar: CookieJar,
) -> Result<Json<SessionDto>, ApiError> {
    let password_set = state.app.auth().password_set()?;
    let csrf_token = jar
        .get(SESSION_COOKIE)
        .and_then(|cookie| state.auth.csrf_if_valid(cookie.value()))
        .filter(|_| password_set);
    Ok(Json(SessionDto {
        password_set,
        logged_in: csrf_token.is_some(),
        csrf_token,
    }))
}

#[utoipa::path(post, path = "/api/v1/auth/login", tag = "auth", request_body = LoginBody,
    responses(
        (status = 200, description = "Đặt cookie mh_session", body = CsrfDto),
        (status = 401, body = ErrorBody),
        (status = 429, body = ErrorBody)
    ))]
pub async fn login(
    State(state): State<ApiState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    jar: CookieJar,
    ApiJson(body): ApiJson<LoginBody>,
) -> Response {
    let ip = peer.ip();
    if let Some(wait) = state.auth.retry_after_secs(ip) {
        return rate_limited(wait);
    }
    // IP này đang được băm, hoặc hết chỗ: bảo thử lại sau, không xếp hàng (hàng chờ cũng tốn RAM).
    let Some(slot) = AuthGate::claim(&state.auth, ip) else {
        return rate_limited(1);
    };
    // Lần thử trước của IP này có thể vừa ghi lần sai thứ 5 ngay trước khi nhả chỗ.
    if let Some(wait) = state.auth.retry_after_secs(ip) {
        return rate_limited(wait);
    }
    let password = body.password;
    let (app, gate) = (Arc::clone(&state.app), Arc::clone(&state.auth));
    // Argon2 tốn vài chục mili giây; không giữ mutex của phiên trong lúc băm. Ghi kết quả rồi mới nhả
    // chỗ, ngay trong luồng blocking, để request bị hủy giữa chừng vẫn được tính.
    let checked = blocking(move || {
        let _slot = slot;
        let ok = app.auth().verify(&password)?;
        if ok {
            gate.clear_failures(ip);
        } else {
            gate.record_failure(ip);
        }
        Ok(ok)
    })
    .await;
    match checked {
        Ok(true) => {}
        Ok(false) => return invalid_password(),
        Err(err) => return err.into_response(),
    }
    match state.auth.open_session() {
        Ok((token, csrf_token)) => (
            jar.add(session_cookie(&token)),
            Json(CsrfDto { csrf_token }),
        )
            .into_response(),
        Err(err) => err.into_response(),
    }
}

#[utoipa::path(post, path = "/api/v1/auth/logout", tag = "auth",
    responses((status = 204, description = "Đã xóa phiên"), (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody)))]
pub async fn logout(State(state): State<ApiState>, jar: CookieJar) -> (CookieJar, StatusCode) {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        state.auth.remove(cookie.value());
    }
    (clear_session_cookie(jar), StatusCode::NO_CONTENT)
}

#[utoipa::path(put, path = "/api/v1/auth/password", tag = "auth", request_body = PasswordBody,
    responses(
        (status = 204, description = "Đã đặt mật khẩu và hủy mọi phiên"),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody)
    ))]
pub async fn put_password(
    State(state): State<ApiState>,
    jar: CookieJar,
    ApiJson(body): ApiJson<PasswordBody>,
) -> Result<(CookieJar, StatusCode), ApiError> {
    let app = Arc::clone(&state.app);
    let current = body.current;
    let next = body.new;
    change_password(&state, jar, move || {
        app.auth().set_password(current.as_deref(), &next)
    })
    .await
}

#[utoipa::path(
    delete,
    path = "/api/v1/auth/password",
    tag = "auth",
    request_body = ClearPasswordBody,
    responses(
        (status = 204, description = "Đã xóa mật khẩu, tắt LAN và hủy mọi phiên"),
        (status = 401, body = ErrorBody)
    )
)]
pub async fn delete_password(
    State(state): State<ApiState>,
    jar: CookieJar,
    ApiJson(body): ApiJson<ClearPasswordBody>,
) -> Result<(CookieJar, StatusCode), ApiError> {
    let app = Arc::clone(&state.app);
    let current = body.current;
    change_password(&state, jar, move || app.auth().clear_password(&current)).await
}

async fn change_password(
    state: &ApiState,
    jar: CookieJar,
    work: impl FnOnce() -> Result<(), AppError> + Send + 'static,
) -> Result<(CookieJar, StatusCode), ApiError> {
    blocking(work).await?;
    state.auth.clear_sessions();
    // Xóa mật khẩu thì LAN tắt theo: đóng listener ngay, không chờ lần kiểm tra định kỳ.
    state.lan.wake();
    Ok((clear_session_cookie(jar), StatusCode::NO_CONTENT))
}

/// Không log `JoinError`: payload của panic có thể chứa mật khẩu.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| ApiError::internal("task mật khẩu dừng"))?
        .map_err(Into::into)
}

fn is_public(path: &str) -> bool {
    // `nest` cắt tiền tố trước khi middleware của router con thấy URI.
    matches!(
        path.trim_end_matches('/'),
        "/auth/session" | "/auth/login" | "/api/v1/auth/session" | "/api/v1/auth/login"
    )
}

fn session_csrf(auth: &AuthGate, headers: &HeaderMap) -> Option<String> {
    let jar = CookieJar::from_headers(headers);
    let cookie = jar.get(SESSION_COOKIE)?;
    auth.csrf_if_valid(cookie.value())
}

fn csrf_matches(expected: &str, headers: &HeaderMap) -> bool {
    let Some(given) = headers
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    expected.as_bytes().ct_eq(given.as_bytes()).into()
}

fn session_cookie(token: &str) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, token.to_owned()))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        // Không có `Max-Age` thì trình duyệt bỏ cookie khi đóng cửa sổ, sớm hơn hạn 30 ngày ở server.
        .max_age(time::Duration::days(SESSION_DAYS as i64))
        .build()
}

fn clear_session_cookie(jar: CookieJar) -> CookieJar {
    jar.remove(Cookie::build(SESSION_COOKIE).path("/"))
}

fn rate_limited(retry_after_s: u64) -> Response {
    let mut response = ApiError::new(StatusCode::TOO_MANY_REQUESTS, ErrorCode::AuthRateLimited)
        .with_params(serde_json::json!({ "retry_after_s": retry_after_s }))
        .into_response();
    if let Ok(value) = HeaderValue::try_from(retry_after_s.to_string()) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    }
    response
}

fn invalid_password() -> Response {
    ApiError::new(StatusCode::UNAUTHORIZED, ErrorCode::AuthInvalid).into_response()
}
