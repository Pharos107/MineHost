//! Route `/settings/curseforge`: vòng đời key và đảm bảo key không lọt ra response, `Debug`, log.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::io;
use std::sync::{Arc, Mutex};

use axum::http::{Method, StatusCode};
use mh_core::CurseforgeKey;
use mh_platform::DataDirs;
use serde_json::json;
use tracing::instrument::WithSubscriber;

use crate::auth_tests::support::{PASSWORD, api, call, login};
use crate::curseforge::CurseforgeKeyBody;

#[derive(Clone, Default)]
struct LogBuf(Arc<Mutex<Vec<u8>>>);

struct BufWriter(Arc<Mutex<Vec<u8>>>);

impl io::Write for BufWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogBuf {
    type Writer = BufWriter;
    fn make_writer(&'a self) -> Self::Writer {
        BufWriter(Arc::clone(&self.0))
    }
}

fn router_with_builtin(
    builtin: Option<&str>,
) -> (tempfile::TempDir, Arc<mh_app::App>, axum::Router) {
    let api = api();
    // Tạo `App` mới có `with_curseforge_key` vì `App::with_curseforge_key` nhận `self`.
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let mut app = mh_app::App::with_deps(
        dirs,
        mh_app::AppDeps {
            catalog: Arc::new(DummyCatalog),
            java: Arc::new(DummyJava),
            port_free: Arc::new(|_| true),
            free_space: Arc::new(|_| None),
            total_memory_mb: 8192,
            supervisor: mh_supervisor::SupervisorConfig::default(),
            save_flush_timeout: std::time::Duration::from_secs(60),
            metrics: mh_app::MetricsConfig::default(),
        },
    )
    .unwrap();
    drop(api);
    if let Some(raw) = builtin {
        app = app.with_curseforge_key(CurseforgeKey::parse(raw).unwrap());
    }
    let app = Arc::new(app);
    let router = crate::router(Arc::clone(&app), 7878);
    (tmp, app, router)
}

struct DummyCatalog;
impl mh_app::ServerCatalog for DummyCatalog {
    fn versions(
        &self,
        _: mh_core::LoaderKind,
    ) -> mh_app::sources::BoxFut<'_, Result<Vec<mh_app::CatalogVersion>, mh_app::AppError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn resolve<'a>(
        &'a self,
        _: mh_core::LoaderKind,
        _: &'a mh_core::McVersion,
    ) -> mh_app::sources::BoxFut<'a, Result<mh_app::sources::ResolvedServer, mh_app::AppError>>
    {
        Box::pin(async {
            Err(mh_app::AppError::new(
                mh_core::ErrorCode::Internal,
                "unused",
            ))
        })
    }
    fn download<'a>(
        &'a self,
        _: &'a mh_app::sources::ServerJar,
        _: &'a std::path::Path,
        _: mh_app::sources::ProgressFn<'a>,
        _: &'a mh_app::sources::CancellationToken,
    ) -> mh_app::sources::BoxFut<'a, Result<(), mh_app::AppError>> {
        Box::pin(async {
            Err(mh_app::AppError::new(
                mh_core::ErrorCode::Internal,
                "unused",
            ))
        })
    }
}

struct DummyJava;
impl mh_app::JavaSource for DummyJava {
    fn ensure<'a>(
        &'a self,
        major: u16,
        _: mh_app::sources::ProgressFn<'a>,
        _: &'a mh_app::sources::CancellationToken,
    ) -> mh_app::sources::BoxFut<'a, Result<mh_app::sources::JavaRuntime, mh_app::AppError>> {
        Box::pin(async move {
            Ok(mh_app::sources::JavaRuntime {
                major,
                home: std::path::PathBuf::from("none"),
                java_bin: std::path::PathBuf::from("none/java"),
                version: "21".to_owned(),
            })
        })
    }
}

#[tokio::test]
async fn routes_reflect_none_builtin_user_and_never_leak_key() {
    let builtin_secret = "$2a$10$BUILTIN_SECRET_CF_KEY_12345";
    let user_secret = "$2a$10$USER_SECRET_CF_KEY_67890";

    let body_debug = format!(
        "{:?}",
        CurseforgeKeyBody {
            api_key: user_secret.to_owned()
        }
    );
    assert!(!body_debug.contains(user_secret));
    assert!(body_debug.contains("***"));

    let logs = LogBuf::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(logs.clone())
        .finish();

    async {
        // 1. Không có key nhúng -> available: false, source: none.
        let (_tmp1, _app1, router_none) = router_with_builtin(None);
        let r = call(
            &router_none,
            Method::GET,
            "/api/v1/settings/curseforge",
            None,
            None,
            None,
        )
        .await;
        assert_eq!(r.status, StatusCode::OK);
        assert_eq!(r.json(), json!({ "available": false, "source": "none" }));

        // Key rỗng/khoảng trắng bị từ chối với INVALID_INPUT.
        let r_bad = call(
            &router_none,
            Method::PUT,
            "/api/v1/settings/curseforge",
            Some(json!({ "api_key": "   \t" })),
            None,
            None,
        )
        .await;
        assert_eq!(r_bad.status, StatusCode::BAD_REQUEST);
        assert_eq!(r_bad.json()["code"], "INVALID_INPUT");
        assert_eq!(r_bad.json()["params"]["field"], "api_key");

        // Đặt key người dùng -> available: true, source: user.
        let r_put = call(
            &router_none,
            Method::PUT,
            "/api/v1/settings/curseforge",
            Some(json!({ "api_key": user_secret })),
            None,
            None,
        )
        .await;
        assert_eq!(r_put.status, StatusCode::OK);
        assert_eq!(r_put.json(), json!({ "available": true, "source": "user" }));
        assert!(!r_put.json().to_string().contains(user_secret));

        // Xóa -> quay về none.
        let r_del = call(
            &router_none,
            Method::DELETE,
            "/api/v1/settings/curseforge",
            None,
            None,
            None,
        )
        .await;
        assert_eq!(r_del.status, StatusCode::OK);
        assert_eq!(
            r_del.json(),
            json!({ "available": false, "source": "none" })
        );

        // 2. Có key nhúng -> builtin, ghi đè -> user, xóa -> về builtin.
        let (_tmp2, app2, router_builtin) = router_with_builtin(Some(builtin_secret));
        let r_get = call(
            &router_builtin,
            Method::GET,
            "/api/v1/settings/curseforge",
            None,
            None,
            None,
        )
        .await;
        assert_eq!(r_get.status, StatusCode::OK);
        assert_eq!(
            r_get.json(),
            json!({ "available": true, "source": "builtin" })
        );
        assert!(!r_get.json().to_string().contains(builtin_secret));

        let r_put2 = call(
            &router_builtin,
            Method::PUT,
            "/api/v1/settings/curseforge",
            Some(json!({ "api_key": user_secret })),
            None,
            None,
        )
        .await;
        assert_eq!(r_put2.status, StatusCode::OK);
        assert_eq!(
            r_put2.json(),
            json!({ "available": true, "source": "user" })
        );
        let r_get2 = call(
            &router_builtin,
            Method::GET,
            "/api/v1/settings/curseforge",
            None,
            None,
            None,
        )
        .await;
        assert_eq!(
            r_get2.json(),
            json!({ "available": true, "source": "user" })
        );
        assert!(!r_get2.json().to_string().contains(user_secret));
        assert!(!r_get2.json().to_string().contains(builtin_secret));

        let r_del2 = call(
            &router_builtin,
            Method::DELETE,
            "/api/v1/settings/curseforge",
            None,
            None,
            None,
        )
        .await;
        assert_eq!(r_del2.status, StatusCode::OK);
        assert_eq!(
            r_del2.json(),
            json!({ "available": true, "source": "builtin" })
        );
        assert!(!r_del2.json().to_string().contains(builtin_secret));
        assert!(!format!("{app2:?}").contains(builtin_secret));
    }
    .with_subscriber(subscriber)
    .await;

    let log_text = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    assert!(
        !log_text.contains(builtin_secret),
        "lộ key nhúng: {log_text}"
    );
    assert!(
        !log_text.contains(user_secret),
        "lộ key người dùng: {log_text}"
    );
}

#[tokio::test]
async fn settings_curseforge_requires_session_and_csrf_when_password_set() {
    let api = api();
    api.app.auth().set_password(None, PASSWORD).unwrap();

    for (method, body) in [
        (Method::GET, None),
        (Method::PUT, Some(json!({ "api_key": "k" }))),
        (Method::DELETE, None),
    ] {
        let r = call(
            &api.router,
            method,
            "/api/v1/settings/curseforge",
            body,
            None,
            None,
        )
        .await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED);
        assert_eq!(r.json()["code"], "AUTH_REQUIRED");
    }

    let (cookie, csrf) = login(&api.router, PASSWORD).await;
    let r_no_csrf = call(
        &api.router,
        Method::PUT,
        "/api/v1/settings/curseforge",
        Some(json!({ "api_key": "secret-k" })),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(r_no_csrf.status, StatusCode::FORBIDDEN);

    let r_ok = call(
        &api.router,
        Method::PUT,
        "/api/v1/settings/curseforge",
        Some(json!({ "api_key": "secret-k" })),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(r_ok.status, StatusCode::OK);
    assert_eq!(r_ok.json(), json!({ "available": true, "source": "user" }));
}
