//! Hỗ trợ test Job cài modpack (`App::create_from_modpack`).

#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mh_app::sources::{
    BoxFut, CatalogVersion, InstallPlan, LoaderInstall, PlannedFile, ProgressFn, ResolvedServer,
};
use mh_app::{
    App, AppDeps, AppError, ContentCatalog, JavaSource, JobSnapshot, JobState, MetricsConfig,
    ModpackRequest, ModpackSourceInput, ServerCatalog,
};
use mh_core::instance::JarName;
use mh_core::{ErrorCode, Launch, LoaderKind, LoaderVersion, McVersion, ServerId, ServerName};
use mh_java::JavaRuntime;
use mh_net::{CancellationToken, Checksum, HostAllowlist, hosts};
use mh_platform::DataDirs;
use mh_providers::ServerJar;
use mh_providers::modpack::PackFile;
use mh_providers::modrinth::{
    FileHashes, ModrinthVersion, Project, SearchFilters, SearchResult, VersionFile,
};
use mh_store::sha1_hex;
use mh_supervisor::SupervisorConfig;
use serde_json::{Value, json};
use sha2::{Digest, Sha512};
use time::OffsetDateTime;
use time::macros::datetime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use zip::write::{SimpleFileOptions, ZipWriter};

pub const WAIT: Duration = Duration::from_secs(20);

pub struct FakeLoaderCatalog;

impl ServerCatalog for FakeLoaderCatalog {
    fn versions(&self, _: LoaderKind) -> BoxFut<'_, Result<Vec<CatalogVersion>, AppError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn resolve<'a>(
        &'a self,
        _: LoaderKind,
        _: &'a McVersion,
    ) -> BoxFut<'a, Result<ResolvedServer, AppError>> {
        Box::pin(async { Err(AppError::new(ErrorCode::InvalidInput, "chỉ test fabric")) })
    }

    fn download<'a>(
        &'a self,
        _: &'a ServerJar,
        _: &'a Path,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async { Ok(()) })
    }

    fn plan<'a>(
        &'a self,
        loader: LoaderKind,
        minecraft: &'a McVersion,
        wanted: Option<&'a LoaderVersion>,
    ) -> BoxFut<'a, Result<LoaderInstall, AppError>> {
        let ver = wanted
            .cloned()
            .unwrap_or_else(|| LoaderVersion::parse("0.16.9").unwrap());
        Box::pin(async move {
            if loader != LoaderKind::Fabric {
                return Err(AppError::new(ErrorCode::InvalidInput, "chỉ test fabric"));
            }
            let jar = "fabric-server-launch.jar";
            Ok(LoaderInstall {
                java_major: 21,
                plan: InstallPlan {
                    minecraft: minecraft.clone(),
                    loader_version: ver,
                    files: vec![PlannedFile {
                        url: "https://maven.fabricmc.net/fake-launch.jar".to_owned(),
                        checksum: Checksum::sha256(&"0".repeat(64)).unwrap(),
                        size: Some(3),
                        dest: jar.to_owned(),
                        hosts: hosts::LOADERS,
                    }],
                    installer: None,
                    launch: Launch::Jar(JarName::parse(jar).unwrap()),
                },
            })
        })
    }

    fn download_file<'a>(
        &'a self,
        _: &'a PlannedFile,
        dest: &'a Path,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            std::fs::write(dest, b"jar").unwrap();
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
            Ok(JavaRuntime {
                major,
                home: PathBuf::from("none"),
                java_bin: PathBuf::from("none/java"),
                version: "21.0.5".to_owned(),
            })
        })
    }
}

#[derive(Default)]
pub struct PackTestCatalog {
    pub versions: HashMap<String, ModrinthVersion>,
    pub by_project: HashMap<String, Vec<ModrinthVersion>>,
    pub files: HashMap<String, Vec<u8>>,
    pub delay: Duration,
    pub in_flight: AtomicUsize,
    pub max_in_flight: AtomicUsize,
    pub hosts: Option<HostAllowlist>,
}

impl PackTestCatalog {
    pub fn add_pack_version(
        &mut self,
        pid: &str,
        vid: &str,
        vnum: &str,
        url: &str,
        mrpack: Vec<u8>,
    ) {
        let vf = VersionFile {
            url: url.to_owned(),
            filename: format!("{vid}.mrpack"),
            size: mrpack.len() as u64,
            hashes: FileHashes {
                sha1: sha1_hex(&mrpack),
                sha512: sha512_hex(&mrpack),
            },
            primary: true,
        };
        let mv = ModrinthVersion {
            id: vid.to_owned(),
            project_id: pid.to_owned(),
            name: format!("Pack {vnum}"),
            version_number: vnum.to_owned(),
            version_type: "release".to_owned(),
            game_versions: vec!["1.21.4".to_owned()],
            loaders: vec!["fabric".to_owned()],
            date_published: datetime!(2025-01-10 0:00 UTC),
            files: vec![vf],
            dependencies: Vec::new(),
        };
        self.files.insert(url.to_owned(), mrpack);
        self.versions.insert(vid.to_owned(), mv.clone());
        self.by_project.entry(pid.to_owned()).or_default().push(mv);
    }
}

impl ContentCatalog for PackTestCatalog {
    fn search<'a>(
        &'a self,
        _: &'a str,
        _: &'a SearchFilters,
    ) -> BoxFut<'a, Result<SearchResult, AppError>> {
        Box::pin(async { Err(AppError::new(ErrorCode::NetworkUnavailable, "không dùng")) })
    }

    fn project<'a>(&'a self, _: &'a str) -> BoxFut<'a, Result<Project, AppError>> {
        Box::pin(async { Err(AppError::new(ErrorCode::NetworkUnavailable, "không dùng")) })
    }

    fn versions<'a>(
        &'a self,
        _: &'a str,
        _: LoaderKind,
        _: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<ModrinthVersion>, AppError>> {
        Box::pin(async { Ok(Vec::new()) })
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
            let bytes = self
                .files
                .get(&file.url)
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, &file.url))?;
            std::fs::write(dest, bytes).unwrap();
            progress(bytes.len() as u64, Some(bytes.len() as u64));
            Ok(())
        })
    }

    fn project_versions<'a>(
        &'a self,
        project: &'a str,
    ) -> BoxFut<'a, Result<Vec<ModrinthVersion>, AppError>> {
        Box::pin(async move { Ok(self.by_project.get(project).cloned().unwrap_or_default()) })
    }

    fn download_pack_file<'a>(
        &'a self,
        file: &'a PackFile,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            let cur = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_in_flight.fetch_max(cur, Ordering::SeqCst);
            if !self.delay.is_zero() {
                tokio::time::sleep(self.delay).await;
            }
            let res = async {
                let url = file
                    .urls
                    .first()
                    .ok_or_else(|| AppError::new(ErrorCode::DownloadFailed, "thiếu url"))?;
                let bytes = self
                    .files
                    .get(url)
                    .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, url))?;
                if sha1_hex(bytes) != file.sha1
                    || Some(&file.checksum) != Checksum::sha512(&sha512_hex(bytes)).as_ref()
                {
                    return Err(AppError::new(ErrorCode::ChecksumMismatch, "sai checksum"));
                }
                std::fs::write(dest, bytes).unwrap();
                progress(bytes.len() as u64, Some(bytes.len() as u64));
                Ok(())
            }
            .await;
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            res
        })
    }

    fn pack_hosts(&self) -> HostAllowlist {
        self.hosts.unwrap_or(hosts::MODRINTH)
    }
}

pub fn sha512_hex(bytes: &[u8]) -> String {
    Sha512::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn make_mrpack(name: &str, files: &[Value], extras: &[(&str, &[u8])]) -> Vec<u8> {
    let index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0.0",
        "name": name,
        "dependencies": { "minecraft": "1.21.4", "fabric-loader": "0.16.9" },
        "files": files,
    });
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default();
    zip.start_file("modrinth.index.json", opts).unwrap();
    zip.write_all(index.to_string().as_bytes()).unwrap();
    for (path, data) in extras {
        zip.start_file(*path, opts).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

pub fn pack_entry(path: &str, url: &str, bytes: &[u8], server_env: Option<&str>) -> Value {
    let mut obj = json!({
        "path": path,
        "hashes": { "sha1": sha1_hex(bytes), "sha512": sha512_hex(bytes) },
        "downloads": [url],
        "fileSize": bytes.len() as u64,
    });
    if let Some(env) = server_env {
        obj["env"] = json!({ "client": "required", "server": env });
    }
    obj
}

pub fn make_app(
    total_memory_mb: u64,
    content: Arc<dyn ContentCatalog>,
) -> (tempfile::TempDir, Arc<App>) {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let deps = AppDeps {
        catalog: Arc::new(FakeLoaderCatalog),
        java: Arc::new(FakeJava),
        port_free: Arc::new(|_| true),
        free_space: Arc::new(|_| Some(100 * 1024 * 1024 * 1024)),
        total_memory_mb,
        supervisor: SupervisorConfig {
            stop_timeout: Duration::from_millis(500),
            ..SupervisorConfig::default()
        },
        save_flush_timeout: Duration::from_secs(60),
        metrics: MetricsConfig::default(),
    };
    (
        tmp,
        Arc::new(App::with_deps(dirs, deps).unwrap().with_content(content)),
    )
}

pub async fn finish(app: &App, job_id: u64) -> JobSnapshot {
    let mut rx = app.jobs().subscribe(job_id).unwrap();
    tokio::time::timeout(WAIT, rx.wait_for(|s| s.state != JobState::Running))
        .await
        .unwrap()
        .unwrap()
        .clone()
}

pub fn modrinth_req(project_id: &str, version_id: Option<&str>, name: &str) -> ModpackRequest {
    ModpackRequest {
        source: ModpackSourceInput::Modrinth {
            project_id: project_id.to_owned(),
            version_id: version_id.map(str::to_owned),
        },
        name: ServerName::parse(name).unwrap(),
        memory_mb: None,
        port: None,
    }
}

pub fn server_root(tmp: &tempfile::TempDir, id: ServerId) -> PathBuf {
    tmp.path().join("servers").join(id.to_string())
}

pub async fn spawn_stub_server(
    listener: TcpListener,
    routes: HashMap<String, (&'static str, Vec<u8>)>,
) -> Arc<Mutex<HashMap<String, usize>>> {
    let counts = Arc::new(Mutex::new(HashMap::<String, usize>::new()));
    let counts_bg = Arc::clone(&counts);
    let routes = Arc::new(routes);
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let (routes, counts) = (Arc::clone(&routes), Arc::clone(&counts_bg));
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("")
                    .to_owned();
                *counts.lock().unwrap().entry(path.clone()).or_default() += 1;
                if let Some((ct, body)) = routes.get(&path) {
                    let hdr = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = sock.write_all(hdr.as_bytes()).await;
                    let _ = sock.write_all(body).await;
                } else {
                    let _ = sock
                        .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                        .await;
                }
            });
        }
    });
    counts
}

pub fn ver_json(
    id: &str,
    pid: &str,
    pub_at: OffsetDateTime,
    file_url: &str,
    filename: &str,
    bytes: &[u8],
) -> Value {
    json!({
        "id": id,
        "project_id": pid,
        "name": id,
        "version_number": "1.2.0",
        "version_type": "release",
        "game_versions": ["1.21.4"],
        "loaders": ["fabric"],
        "date_published": pub_at.format(&time::format_description::well_known::Rfc3339).unwrap(),
        "files": [{
            "url": file_url,
            "filename": filename,
            "size": bytes.len() as u64,
            "hashes": { "sha1": sha1_hex(bytes), "sha512": sha512_hex(bytes) },
            "primary": true
        }],
        "dependencies": []
    })
}
