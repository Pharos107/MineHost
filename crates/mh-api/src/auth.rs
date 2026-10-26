//! Phiên đăng nhập, CSRF và giới hạn đăng nhập sai (spec P2, mục 3.3, 5.8, 6.10, 8).
//!
//! Phiên chỉ nằm trong RAM: tắt MineHost là mất. Cookie không đặt `Secure` vì giao diện đi qua HTTP.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use axum::Json;
use axum::Router;
use axum::extract::{ConnectInfo, FromRef, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use mh_app::{App, AppError};
use mh_core::ErrorCode;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use utoipa::ToSchema;

use crate::error::{ApiError, ApiJson, ErrorBody};
use crate::guard;

pub(crate) const SESSION_COOKIE: &str = "mh_session";
const CSRF_HEADER: HeaderName = HeaderName::from_static("x-csrf-token");
const SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const MAX_FAILURES: u32 = 5;
const FIRST_LOCK_SECS: u64 = 60;
const MAX_LOCK_SECS: u64 = 15 * 60;

#[derive(Clone)]
pub(crate) struct ApiState {
    app: Arc<App>,
    auth: Arc<AuthGate>,
}

impl ApiState {
    pub(crate) fn new(app: Arc<App>) -> Self {
        Self {
            app,
            auth: Arc::new(AuthGate::default()),
        }
    }
}

impl FromRef<ApiState> for Arc<App> {
    fn from_ref(state: &ApiState) -> Self {
        Arc::clone(&state.app)
    }
}

#[derive(Default)]
struct AuthGate {
    sessions: Mutex<HashMap<String, Session>>,
    logins: Mutex<HashMap<IpAddr, Attempt>>,
}

struct Session {
    csrf: String,
    expires_at: SystemTime,
}

struct Attempt {
    failures: u32,
    locked_until: Option<SystemTime>,
    next_lock_secs: u64,
    touched: SystemTime,
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

impl AuthGate {
    fn open_session(&self) -> Result<(String, String), ApiError> {
        let token = random_token()?;
        let csrf = random_token()?;
        let expires_at = SystemTime::now()
            .checked_add(SESSION_TTL)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        lock(&self.sessions).insert(
            token.clone(),
            Session {
                csrf: csrf.clone(),
                expires_at,
            },
        );
        Ok((token, csrf))
    }

    fn csrf_if_valid(&self, token: &str) -> Option<String> {
        let mut sessions = lock(&self.sessions);
        let expires_at = sessions.get(token)?.expires_at;
        if expires_at <= SystemTime::now() {
            sessions.remove(token);
            return None;
        }
        Some(sessions.get(token)?.csrf.clone())
    }

    fn remove(&self, token: &str) {
        lock(&self.sessions).remove(token);
    }

    /// Đổi hoặc xóa mật khẩu thì mọi trình duyệt phải đăng nhập lại.
    fn clear_sessions(&self) {
        lock(&self.sessions).clear();
    }

    fn retry_after_secs(&self, ip: IpAddr) -> Option<u64> {
        let now = SystemTime::now();
        let mut logins = lock(&self.logins);
        prune(&mut logins, now);
        let until = logins.get(&ip)?.locked_until?;
        secs_until(until, now)
    }

    /// Lần sai thứ 5 chỉ ghi nhận. Lần gọi sau, trong lúc còn khóa, mới bị 429.
    fn record_failure(&self, ip: IpAddr) {
        let now = SystemTime::now();
        let mut logins = lock(&self.logins);
        prune(&mut logins, now);
        let attempt = logins.entry(ip).or_insert_with(|| Attempt {
            failures: 0,
            locked_until: None,
            next_lock_secs: FIRST_LOCK_SECS,
            touched: now,
        });
        if attempt.locked_until.is_some_and(|until| until <= now) {
            attempt.locked_until = None;
        }
        attempt.touched = now;
        attempt.failures += 1;
        if attempt.failures < MAX_FAILURES {
            return;
        }
        let lock_secs = attempt.next_lock_secs;
        attempt.locked_until = now.checked_add(Duration::from_secs(lock_secs));
        attempt.next_lock_secs = (lock_secs * 2).min(MAX_LOCK_SECS);
        attempt.failures = 0;
    }

    fn clear_failures(&self, ip: IpAddr) {
        lock(&self.logins).remove(&ip);
    }
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
    if is_public(req.uri().path()) {
        return next.run(req).await;
    }
    let password_set = match state.app.auth().password_set() {
        Ok(set) => set,
        Err(err) => return ApiError::from(err).into_response(),
    };
    if !password_set {
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
    let password = body.password;
    let app = Arc::clone(&state.app);
    // Argon2 tốn vài chục mili giây; không giữ mutex của phiên trong lúc băm.
    let ok = match blocking(move || app.auth().verify(&password)).await {
        Ok(ok) => ok,
        Err(err) => return err.into_response(),
    };
    if !ok {
        state.auth.record_failure(ip);
        return invalid_password();
    }
    state.auth.clear_failures(ip);
    match state.auth.open_session() {
        Ok((token, csrf_token)) => {
            (jar.add(session_cookie(&token)), Json(CsrfDto { csrf_token })).into_response()
        }
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
        .build()
}

fn clear_session_cookie(jar: CookieJar) -> CookieJar {
    jar.remove(Cookie::build(SESSION_COOKIE).path("/"))
}

fn random_token() -> Result<String, ApiError> {
    let mut bytes = [0u8; 32];
    // `fill_bytes` panic khi OS không có entropy. Trả lỗi thay vì làm sập request.
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(ApiError::internal)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
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

/// Bản ghi không còn khóa và lâu không bị đụng thì bỏ, để bảng IP không phình.
fn prune(logins: &mut HashMap<IpAddr, Attempt>, now: SystemTime) {
    let keep = Duration::from_secs(MAX_LOCK_SECS);
    logins.retain(|_, attempt| {
        if attempt.locked_until.is_some_and(|until| until > now) {
            return true;
        }
        let anchor = attempt.locked_until.unwrap_or(attempt.touched);
        now.duration_since(anchor).is_ok_and(|age| age < keep)
    });
}

fn secs_until(until: SystemTime, now: SystemTime) -> Option<u64> {
    let left = until.duration_since(now).ok()?;
    if left.is_zero() {
        return None;
    }
    Some(left.as_secs() + u64::from(left.subsec_nanos() > 0))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Chỉ bị poison khi một luồng panic lúc đang giữ; dữ liệu bên trong vẫn dùng được.
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}
