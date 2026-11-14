//! Nguồn mod giả và tiện ích dùng chung cho các test nội dung.
//!
//! Mỗi file test chỉ dùng một phần các hàm ở đây, nên tắt cảnh báo hàm không dùng.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use mh_app::content::{
    ContentCatalog, Dependency, DependencyType, FileHashes, ModrinthVersion, Project,
    SearchFilters, SearchResult, SideSupport, VersionFile,
};
use mh_app::sources::{BoxFut, CatalogVersion, ProgressFn, ResolvedServer};
use mh_app::{App, AppDeps, AppError, JavaSource, JobState, MetricsConfig, ServerCatalog};
use mh_core::instance::GameSection;
use mh_core::{
    ErrorCode, GamePort, InstanceSpec, LoaderKind, McVersion, MemoryLimits, ServerId, ServerName,
};
use mh_java::JavaRuntime;
use mh_net::{CancellationToken, Checksum};
use mh_platform::DataDirs;
use mh_providers::ServerJar;
use mh_providers::modrinth::{ProjectLinks, modrinth_loaders};
use mh_store::ServerStore;
use mh_supervisor::SupervisorConfig;
use sha2::{Digest, Sha512};
use time::macros::datetime;
use zip::write::SimpleFileOptions;

pub const WAIT: Duration = Duration::from_secs(10);

pub struct FakeServerCatalog;

impl ServerCatalog for FakeServerCatalog {
    fn versions(&self, _: LoaderKind) -> BoxFut<'_, Result<Vec<CatalogVersion>, AppError>> {
        Box::pin(async {
            Ok(vec![CatalogVersion {
                id: McVersion::parse("1.21.4").unwrap(),
                release_time: datetime!(2024-12-03 10:12:57 UTC),
            }])
        })
    }

    fn resolve<'a>(
        &'a self,
        _: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<ResolvedServer, AppError>> {
        Box::pin(async move {
            Ok(ResolvedServer {
                minecraft: minecraft.clone(),
                build: Some(1),
                java_major: 21,
                jar: ServerJar {
                    url: "https://example.invalid/paper.jar".to_owned(),
                    checksum: Checksum::sha256(&"0".repeat(64)).unwrap(),
                    size: 8,
                },
            })
        })
    }

    fn download<'a>(
        &'a self,
        _: &'a ServerJar,
        dest: &'a Path,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            std::fs::write(dest, b"fake jar").unwrap();
            Ok(())
        })
    }
}

pub struct FakeJava;

impl JavaSource for FakeJava {
    fn ensure<'a>(
        &'a self,
        major: u16,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<JavaRuntime, AppError>> {
        Box::pin(async move {
            let bin = PathBuf::from(env!("CARGO_BIN_EXE_app-fake-mc"));
            Ok(JavaRuntime {
                major,
                home: bin.parent().unwrap().to_path_buf(),
                java_bin: bin,
                version: "21.0.5".to_owned(),
            })
        })
    }
}

#[derive(Default)]
pub struct FakeModCatalog {
    pub projects: HashMap<String, Project>,
    pub versions: HashMap<String, ModrinthVersion>,
    pub by_project: HashMap<String, Vec<ModrinthVersion>>,
    pub files: HashMap<String, Vec<u8>>,
    pub fail_urls: HashSet<String>,
    pub downloads: AtomicUsize,
}

impl FakeModCatalog {
    pub fn add_project(&mut self, id: &str, slug: &str, title: &str, server_side: SideSupport) {
        let p = Project {
            id: id.to_owned(),
            slug: slug.to_owned(),
            project_type: "mod".to_owned(),
            title: title.to_owned(),
            description: String::new(),
            body: String::new(),
            icon_url: None,
            downloads: 1,
            loaders: vec!["fabric".to_owned()],
            game_versions: vec!["1.21.4".to_owned()],
            client_side: SideSupport::Required,
            server_side,
            links: ProjectLinks {
                source_url: None,
                issues_url: None,
                wiki_url: None,
                discord_url: None,
            },
        };
        self.projects.insert(id.to_owned(), p.clone());
        self.projects.insert(slug.to_owned(), p);
    }

    pub fn add_version(
        &mut self,
        id: &str,
        project_id: &str,
        filename: &str,
        loaders: &[&str],
        deps: Vec<Dependency>,
        bytes: Vec<u8>,
    ) {
        let url = format!("https://cdn.modrinth.com/{id}/{filename}");
        let ver = ModrinthVersion {
            id: id.to_owned(),
            project_id: project_id.to_owned(),
            name: id.to_owned(),
            version_number: id.to_owned(),
            version_type: "release".to_owned(),
            game_versions: vec!["1.21.4".to_owned()],
            loaders: loaders.iter().map(|s| (*s).to_owned()).collect(),
            date_published: datetime!(2026-11-01 00:00 UTC),
            files: vec![VersionFile {
                url: url.clone(),
                filename: filename.to_owned(),
                size: bytes.len() as u64,
                hashes: FileHashes {
                    sha1: "0".repeat(40),
                    sha512: sha512_hex(&bytes),
                },
                primary: true,
            }],
            dependencies: deps,
        };
        self.files.insert(url, bytes);
        self.versions.insert(id.to_owned(), ver.clone());
        self.by_project
            .entry(project_id.to_owned())
            .or_default()
            .push(ver);
    }
}

impl ContentCatalog for FakeModCatalog {
    fn search<'a>(
        &'a self,
        _query: &'a str,
        filters: &'a SearchFilters,
    ) -> BoxFut<'a, Result<SearchResult, AppError>> {
        Box::pin(async move {
            Ok(SearchResult {
                hits: Vec::new(),
                offset: filters.offset,
                limit: filters.limit,
                total_hits: 0,
            })
        })
    }

    fn project<'a>(&'a self, id_or_slug: &'a str) -> BoxFut<'a, Result<Project, AppError>> {
        Box::pin(async move {
            self.projects
                .get(id_or_slug)
                .cloned()
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, id_or_slug))
        })
    }

    fn versions<'a>(
        &'a self,
        project: &'a str,
        loader: LoaderKind,
        mc: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<ModrinthVersion>, AppError>> {
        Box::pin(async move {
            let expected = modrinth_loaders(loader);
            let list = self
                .by_project
                .get(project)
                .map(|v| {
                    v.iter()
                        .filter(|ver| {
                            ver.game_versions.iter().any(|g| g == mc.as_str())
                                && ver.loaders.iter().any(|l| expected.contains(&l.as_str()))
                        })
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            Ok(list)
        })
    }

    fn version<'a>(&'a self, id: &'a str) -> BoxFut<'a, Result<ModrinthVersion, AppError>> {
        Box::pin(async move {
            self.versions
                .get(id)
                .cloned()
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, id))
        })
    }

    fn download<'a>(
        &'a self,
        file: &'a VersionFile,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            self.downloads
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.fail_urls.contains(&file.url) {
                return Err(AppError::new(ErrorCode::DownloadFailed, "lỗi tải giả lập"));
            }
            let bytes = self
                .files
                .get(&file.url)
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, &file.url))?;
            std::fs::write(dest, bytes).unwrap();
            progress(bytes.len() as u64, Some(bytes.len() as u64));
            Ok(())
        })
    }
}

pub fn sha512_hex(bytes: &[u8]) -> String {
    Sha512::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn build_jar(fabric_env: Option<&str>, marker: &str) -> Vec<u8> {
    let cur = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cur);
    let opts = SimpleFileOptions::default();
    if let Some(env) = fabric_env {
        zip.start_file("fabric.mod.json", opts).unwrap();
        let json = format!(
            r#"{{"schemaVersion":1,"id":"test","version":"1.0","name":"Test","environment":"{env}"}}"#
        );
        zip.write_all(json.as_bytes()).unwrap();
    } else {
        zip.start_file("META-INF/MANIFEST.MF", opts).unwrap();
        zip.write_all(marker.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

pub fn dep(pid: &str, vid: Option<&str>, kind: DependencyType) -> Dependency {
    Dependency {
        project_id: Some(pid.to_owned()),
        version_id: vid.map(str::to_owned),
        dependency_type: kind,
    }
}

pub fn test_app(catalog: Arc<dyn ContentCatalog>) -> (tempfile::TempDir, Arc<App>) {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let deps = AppDeps {
        catalog: Arc::new(FakeServerCatalog),
        java: Arc::new(FakeJava),
        port_free: Arc::new(|_| true),
        free_space: Arc::new(|_| Some(100 * 1024 * 1024 * 1024)),
        total_memory_mb: 16 * 1024,
        supervisor: SupervisorConfig {
            stop_timeout: Duration::from_millis(500),
            ..SupervisorConfig::default()
        },
        save_flush_timeout: Duration::from_secs(60),
        metrics: MetricsConfig::default(),
    };
    let app = App::with_deps(dirs, deps).unwrap().with_content(catalog);
    (tmp, Arc::new(app))
}

pub fn make_server(app: &App, loader: LoaderKind) -> ServerId {
    let store = ServerStore::new(app.dirs().servers(), app.dirs().trash());
    let id = ServerId::generate();
    store.create_dir(id).unwrap();
    let game = GameSection::new(loader, McVersion::parse("1.21.4").unwrap(), Some(1), 21);
    let spec = InstanceSpec::new(
        id,
        ServerName::parse("Test").unwrap(),
        datetime!(2026-11-14 00:00 UTC),
        game,
        MemoryLimits::new(1024, 2048).unwrap(),
        GamePort::new(25565).unwrap(),
    );
    store.save(&spec).unwrap();
    id
}

pub async fn finish_job(app: &App, job_id: u64) -> mh_app::JobSnapshot {
    let mut rx = app.jobs().subscribe(job_id).unwrap();
    tokio::time::timeout(WAIT, rx.wait_for(|s| s.state != JobState::Running))
        .await
        .unwrap()
        .unwrap()
        .clone()
}

pub async fn collect_steps(app: &App, job_id: u64) -> (mh_app::JobSnapshot, Vec<String>) {
    let mut rx = app.jobs().subscribe(job_id).unwrap();
    let mut seen = Vec::new();
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let cur = rx.borrow_and_update().clone();
        if let Some(p) = &cur.progress
            && let Ok(v) = serde_json::to_value(&p.step)
            && let Some(k) = v.get("kind").and_then(|s| s.as_str())
            && seen.last().is_none_or(|prev| prev != k)
        {
            seen.push(k.to_owned());
        }
        if cur.state != JobState::Running {
            return (cur, seen);
        }
        tokio::time::timeout_at(deadline, rx.changed())
            .await
            .unwrap()
            .unwrap();
    }
}
