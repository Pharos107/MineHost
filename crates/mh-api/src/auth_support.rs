//! Khung `oneshot` cho test phiên. Tách khỏi `auth_tests.rs` để mỗi file dưới 500 dòng.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use mh_app::sources::{
    BoxFut, CancellationToken, CatalogVersion, JavaRuntime, ProgressFn, ResolvedServer, ServerJar,
};
use mh_app::{App, AppDeps, AppError, JavaSource, ServerCatalog};
use mh_core::{ErrorCode, LoaderKind, McVersion};
use mh_platform::DataDirs;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::auth::SESSION_COOKIE;

pub(super) const HOST: &str = "127.0.0.1:7878";
pub(super) const ORIGIN: &str = "http://127.0.0.1:7878";
pub(super) const PASSWORD: &str = "correct-horse";
pub(super) const NEXT: &str = "battery-staple";

struct UnusedCatalog;

impl ServerCatalog for UnusedCatalog {
    fn versions(&self, _: LoaderKind) -> BoxFut<'_, Result<Vec<CatalogVersion>, AppError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn resolve<'a>(
        &'a self,
        _: LoaderKind,
        _: &'a McVersion,
    ) -> BoxFut<'a, Result<ResolvedServer, AppError>> {
        Box::pin(async { Err(AppError::new(ErrorCode::Internal, "test không tải server")) })
    }

    fn download<'a>(
        &'a self,
        _: &'a ServerJar,
        _: &'a Path,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async { Err(AppError::new(ErrorCode::Internal, "test không tải server")) })
    }
}

struct UnusedJava;

impl JavaSource for UnusedJava {
    fn ensure<'a>(
        &'a self,
        major: u16,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<JavaRuntime, AppError>> {
        Box::pin(async move {
            Ok(JavaRuntime {
                major,
                home: PathBuf::from("none"),
                java_bin: PathBuf::from("none/java"),
                version: "21".to_owned(),
            })
        })
    }
}

pub(super) struct Api {
    _tmp: tempfile::TempDir,
    pub(super) app: Arc<App>,
    pub(super) router: axum::Router,
}

pub(super) fn api() -> Api {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let deps = AppDeps {
        catalog: Arc::new(UnusedCatalog),
        java: Arc::new(UnusedJava),
        port_free: Arc::new(|_| true),
        free_space: Arc::new(|_| None),
        total_memory_mb: 8192,
        supervisor: mh_supervisor::SupervisorConfig::default(),
    };
    let app = Arc::new(App::with_deps(dirs, deps).unwrap());
    let router = crate::router(Arc::clone(&app), 7878);
    Api {
        _tmp: tmp,
        app,
        router,
    }
}

pub(super) struct Reply {
    pub(super) status: StatusCode,
    pub(super) headers: axum::http::HeaderMap,
    bytes: Vec<u8>,
}

impl Reply {
    pub(super) fn json(&self) -> Value {
        serde_json::from_slice(&self.bytes).unwrap_or(Value::Null)
    }

    pub(super) fn set_cookie(&self) -> Option<&str> {
        self.headers
            .get(header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
    }
}

pub(super) async fn call(
    router: &axum::Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    cookie: Option<&str>,
    csrf: Option<&str>,
) -> Reply {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, HOST)
        .header(header::ORIGIN, ORIGIN);
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    if let Some(csrf) = csrf {
        builder = builder.header("x-csrf-token", csrf);
    }
    let body = match body {
        Some(json) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(json.to_string())
        }
        None => Body::empty(),
    };
    let mut request = builder.body(body).unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 9))));
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    Reply {
        status,
        headers,
        bytes,
    }
}

fn cookie_header(raw: &str) -> String {
    let (pair, attrs) = raw.split_once(';').unwrap_or((raw, ""));
    let attrs = attrs.to_ascii_lowercase();
    assert!(attrs.contains("httponly"), "{raw}");
    assert!(attrs.contains("samesite=strict"), "{raw}");
    assert!(
        attrs.split(';').any(|part| part.trim() == "path=/"),
        "{raw}"
    );
    assert!(
        !attrs.split(';').any(|part| part.trim() == "secure"),
        "{raw}"
    );
    let (name, value) = pair.trim().split_once('=').unwrap();
    assert_eq!(name, SESSION_COOKIE);
    assert!(!value.is_empty());
    assert!(!value.contains(PASSWORD));
    format!("{SESSION_COOKIE}={value}")
}

pub(super) async fn login(router: &axum::Router, password: &str) -> (String, String) {
    let reply = call(
        router,
        Method::POST,
        "/api/v1/auth/login",
        Some(json!({ "password": password })),
        None,
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json());
    let cookie = cookie_header(reply.set_cookie().unwrap());
    let csrf = reply.json()["csrf_token"].as_str().unwrap().to_owned();
    (cookie, csrf)
}
