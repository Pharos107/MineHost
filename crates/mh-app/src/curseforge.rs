//! Quản lý API key CurseForge (spec P3, mục 3.6, 4.3, 8).
//!
//! Key người dùng lưu ở `<data>/secrets.toml` và luôn được ưu tiên hơn key nhúng lúc build.

use mh_core::{CurseforgeKey, ErrorCode};
use mh_store::Secrets;

use crate::{App, AppError, lock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurseforgeKeySource {
    Builtin,
    User,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurseforgeStatus {
    pub available: bool,
    pub source: CurseforgeKeySource,
}

impl App {
    pub fn with_curseforge_key(mut self, key: CurseforgeKey) -> Self {
        self.curseforge_builtin = Some(key);
        self
    }

    pub fn curseforge_status(&self) -> Result<CurseforgeStatus, AppError> {
        let user_key = self.load_user_curseforge_key()?;
        if user_key.is_some() {
            return Ok(CurseforgeStatus {
                available: true,
                source: CurseforgeKeySource::User,
            });
        }
        Ok(fallback_status(self.curseforge_builtin.is_some()))
    }

    pub fn set_curseforge_key(&self, raw: &str) -> Result<CurseforgeStatus, AppError> {
        let key = CurseforgeKey::parse(raw).map_err(|err| {
            AppError::new(ErrorCode::InvalidInput, err.to_string())
                .with_params(serde_json::json!({ "field": "api_key" }))
        })?;
        let path = self.dirs.secrets_file();
        let _guard = lock(&self.secrets_lock);
        let mut secrets = Secrets::load(&path)?;
        secrets.curseforge_api_key = Some(key);
        secrets.save(&path)?;
        tracing::info!("đã lưu API key CurseForge của người dùng");
        Ok(CurseforgeStatus {
            available: true,
            source: CurseforgeKeySource::User,
        })
    }

    pub fn clear_curseforge_key(&self) -> Result<CurseforgeStatus, AppError> {
        let path = self.dirs.secrets_file();
        {
            let _guard = lock(&self.secrets_lock);
            let mut secrets = Secrets::load(&path)?;
            secrets.curseforge_api_key = None;
            secrets.save(&path)?;
        }
        tracing::info!("đã xóa API key CurseForge của người dùng");
        Ok(fallback_status(self.curseforge_builtin.is_some()))
    }

    /// Key hiệu lực cho provider CurseForge (PR-27): key người dùng nếu có, không thì key nhúng.
    #[allow(dead_code)]
    pub(crate) fn effective_curseforge_key(&self) -> Result<Option<CurseforgeKey>, AppError> {
        let user_key = self.load_user_curseforge_key()?;
        Ok(user_key.or_else(|| self.curseforge_builtin.clone()))
    }

    fn load_user_curseforge_key(&self) -> Result<Option<CurseforgeKey>, AppError> {
        let path = self.dirs.secrets_file();
        let _guard = lock(&self.secrets_lock);
        Ok(Secrets::load(&path)?.curseforge_api_key)
    }
}

const fn fallback_status(has_builtin: bool) -> CurseforgeStatus {
    if has_builtin {
        CurseforgeStatus {
            available: true,
            source: CurseforgeKeySource::Builtin,
        }
    } else {
        CurseforgeStatus {
            available: false,
            source: CurseforgeKeySource::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use mh_core::{LoaderKind, McVersion};
    use mh_platform::DataDirs;
    use mh_supervisor::SupervisorConfig;

    use super::*;
    use crate::AppDeps;
    use crate::sources::{
        BoxFut, CancellationToken, CatalogVersion, JavaRuntime, JavaSource, ProgressFn,
        ResolvedServer, ServerCatalog, ServerJar,
    };

    struct NoopCatalog;
    impl ServerCatalog for NoopCatalog {
        fn versions(&self, _: LoaderKind) -> BoxFut<'_, Result<Vec<CatalogVersion>, AppError>> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn resolve<'a>(
            &'a self,
            _: LoaderKind,
            _: &'a McVersion,
        ) -> BoxFut<'a, Result<ResolvedServer, AppError>> {
            Box::pin(async { Err(AppError::new(ErrorCode::Internal, "unused")) })
        }
        fn download<'a>(
            &'a self,
            _: &'a ServerJar,
            _: &'a Path,
            _: ProgressFn<'a>,
            _: &'a CancellationToken,
        ) -> BoxFut<'a, Result<(), AppError>> {
            Box::pin(async { Err(AppError::new(ErrorCode::Internal, "unused")) })
        }
    }

    struct NoopJava;
    impl JavaSource for NoopJava {
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

    fn test_app() -> (tempfile::TempDir, App) {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
        dirs.ensure().unwrap();
        let deps = AppDeps {
            catalog: Arc::new(NoopCatalog),
            java: Arc::new(NoopJava),
            port_free: Arc::new(|_| true),
            free_space: Arc::new(|_| None),
            total_memory_mb: 8192,
            supervisor: SupervisorConfig::default(),
            save_flush_timeout: Duration::from_secs(60),
            metrics: crate::MetricsConfig::default(),
        };
        (tmp, App::with_deps(dirs, deps).unwrap())
    }

    #[tokio::test]
    async fn lifecycle_without_and_with_builtin_key() {
        let (_tmp, app) = test_app();
        assert_eq!(
            app.curseforge_status().unwrap(),
            CurseforgeStatus {
                available: false,
                source: CurseforgeKeySource::None,
            }
        );
        assert_eq!(app.effective_curseforge_key().unwrap(), None);

        let user = "user-secret-cf-key";
        assert_eq!(
            app.set_curseforge_key(user).unwrap(),
            CurseforgeStatus {
                available: true,
                source: CurseforgeKeySource::User,
            }
        );
        assert_eq!(
            app.effective_curseforge_key()
                .unwrap()
                .as_ref()
                .map(CurseforgeKey::expose),
            Some(user)
        );
        assert_eq!(
            app.clear_curseforge_key().unwrap(),
            CurseforgeStatus {
                available: false,
                source: CurseforgeKeySource::None,
            }
        );
        assert_eq!(app.effective_curseforge_key().unwrap(), None);

        let builtin = "builtin-secret-cf-key";
        let app = app.with_curseforge_key(CurseforgeKey::parse(builtin).unwrap());
        assert_eq!(
            app.curseforge_status().unwrap(),
            CurseforgeStatus {
                available: true,
                source: CurseforgeKeySource::Builtin,
            }
        );
        assert_eq!(
            app.effective_curseforge_key()
                .unwrap()
                .as_ref()
                .map(CurseforgeKey::expose),
            Some(builtin)
        );

        app.set_curseforge_key(user).unwrap();
        assert_eq!(
            app.curseforge_status().unwrap(),
            CurseforgeStatus {
                available: true,
                source: CurseforgeKeySource::User,
            }
        );
        assert_eq!(
            app.effective_curseforge_key()
                .unwrap()
                .as_ref()
                .map(CurseforgeKey::expose),
            Some(user)
        );

        assert_eq!(
            app.clear_curseforge_key().unwrap(),
            CurseforgeStatus {
                available: true,
                source: CurseforgeKeySource::Builtin,
            }
        );
        assert_eq!(
            app.effective_curseforge_key()
                .unwrap()
                .as_ref()
                .map(CurseforgeKey::expose),
            Some(builtin)
        );
    }

    #[tokio::test]
    async fn rejects_empty_and_keeps_unknown_secrets_and_never_logs_key() {
        let (_tmp, app) = test_app();
        let builtin = "builtin-top-secret-999";
        let user = "user-top-secret-888";
        std::fs::write(app.dirs().secrets_file(), "other_key = \"keep-me\"\n").unwrap();

        let logs = LogBuf::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(logs.clone())
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let app = app.with_curseforge_key(CurseforgeKey::parse(builtin).unwrap());
            let err = app.set_curseforge_key("   ").unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidInput);
            assert_eq!(err.params["field"], "api_key");

            app.set_curseforge_key(user).unwrap();
            let effective = app.effective_curseforge_key().unwrap().unwrap();
            let secrets = Secrets::load(&app.dirs().secrets_file()).unwrap();
            tracing::info!(?app, ?effective, ?secrets, %effective, "kiểm tra log");

            app.clear_curseforge_key().unwrap();
            let kept = Secrets::load(&app.dirs().secrets_file()).unwrap();
            assert_eq!(
                kept.extra.get("other_key"),
                Some(&toml::Value::String("keep-me".into()))
            );
        });

        let text = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
        assert!(!text.contains(builtin), "lộ key nhúng trong log: {text}");
        assert!(!text.contains(user), "lộ key người dùng trong log: {text}");
        assert!(!text.contains("keep-me"), "lộ khóa lạ trong log: {text}");
    }
}
