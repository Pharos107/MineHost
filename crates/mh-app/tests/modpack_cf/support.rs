//! Tiện ích dùng chung cho test tích hợp cài modpack CurseForge trong `mh-app`.

#![allow(clippy::unwrap_used, dead_code)]

use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use mh_app::content::{CurseForge, CurseforgeCatalog};
use mh_app::sources::{
    BoxFut, CancellationToken, CatalogVersion, Checksum, HostAllowlist, InstallPlan, LoaderInstall,
    PlannedFile, ProgressFn, ResolvedServer, ServerJar,
};
use mh_app::{
    App, AppDeps, AppError, JavaSource, JobSnapshot, JobState, MetricsConfig, ServerCatalog,
};
use mh_core::instance::JarName;
use mh_core::{CurseforgeKey, ErrorCode, Launch, LoaderKind, LoaderVersion, McVersion};
use mh_java::JavaRuntime;
use mh_net::{HttpClient, NetConfig};
use mh_platform::DataDirs;
use mh_supervisor::SupervisorConfig;
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use time::macros::datetime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zip::write::{SimpleFileOptions, ZipWriter};

const WAIT: Duration = Duration::from_secs(10);

pub fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn build_jar(environment: &str, mod_id: &str) -> Vec<u8> {
    let meta = format!(
        r#"{{"schemaVersion":1,"id":"{mod_id}","version":"1.0.0","name":"{mod_id}","environment":"{environment}"}}"#
    );
    build_zip(&[("fabric.mod.json", meta.as_bytes())])
}

pub fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().unix_permissions(0o755);
    for &(name, bytes) in entries {
        writer.start_file(name, opts).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

pub fn cf_manifest(
    name: &str,
    version: &str,
    files: &[(u64, u64, bool)],
    overrides: &str,
) -> Vec<u8> {
    let file_json: Vec<Value> = files
        .iter()
        .map(|&(pid, fid, required)| json!({ "projectID": pid, "fileID": fid, "required": required }))
        .collect();
    json!({
        "minecraft": {
            "version": "1.21.4",
            "modLoaders": [{ "id": "fabric-0.16.9", "primary": true }]
        },
        "name": name,
        "version": version,
        "files": file_json,
        "overrides": overrides
    })
    .to_string()
    .into_bytes()
}

pub fn mod_json(id: u64, slug: &str, name: &str, class_id: u64, allow: bool) -> Value {
    json!({
        "id": id,
        "name": name,
        "slug": slug,
        "summary": format!("Mô tả {name}"),
        "classId": class_id,
        "downloadCount": 1000,
        "allowModDistribution": allow,
        "links": {
            "websiteUrl": format!("https://www.curseforge.com/minecraft/mc-mods/{slug}")
        }
    })
}

#[allow(clippy::too_many_arguments)]
pub fn file_json(
    id: u64,
    mod_id: u64,
    name: &str,
    url: Option<String>,
    sha1: &str,
    len: usize,
    sp_id: Option<u64>,
    is_server_pack: bool,
) -> Value {
    json!({
        "id": id,
        "modId": mod_id,
        "displayName": name,
        "fileName": name,
        "releaseType": 1,
        "fileDate": "2026-11-01T00:00:00Z",
        "fileLength": len,
        "downloadUrl": url,
        "hashes": [{ "algo": 1, "value": sha1 }],
        "gameVersions": ["1.21.4", "Fabric"],
        "dependencies": [],
        "serverPackFileId": sp_id,
        "isServerPack": is_server_pack
    })
}

#[derive(Default)]
pub struct CfLoaderCatalog {
    pub plan_calls: AtomicUsize,
}

impl ServerCatalog for CfLoaderCatalog {
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
            std::fs::write(dest, b"paper").unwrap();
            Ok(())
        })
    }

    fn plan<'a>(
        &'a self,
        loader: LoaderKind,
        mc: &'a McVersion,
        wanted: Option<&'a LoaderVersion>,
    ) -> BoxFut<'a, Result<LoaderInstall, AppError>> {
        self.plan_calls.fetch_add(1, Ordering::SeqCst);
        let ver = wanted
            .cloned()
            .unwrap_or_else(|| LoaderVersion::parse("0.16.9").unwrap());
        Box::pin(async move {
            if loader != LoaderKind::Fabric {
                return Err(AppError::new(ErrorCode::InvalidInput, "chỉ hỗ trợ fabric"));
            }
            let jar = "fabric-server-launch.jar";
            Ok(LoaderInstall {
                java_major: 21,
                plan: InstallPlan {
                    minecraft: mc.clone(),
                    loader_version: ver,
                    files: vec![PlannedFile {
                        url: "https://maven.fabricmc.net/fake.jar".to_owned(),
                        checksum: Checksum::sha256(&"0".repeat(64)).unwrap(),
                        size: Some(10),
                        dest: jar.to_owned(),
                        hosts: HostAllowlist::new(&[]),
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
            std::fs::write(dest, b"loader-jar").unwrap();
            Ok(())
        })
    }
}

struct DummyJava;

impl JavaSource for DummyJava {
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

pub fn cf_app(
    server: &MockServer,
    with_key: bool,
) -> (tempfile::TempDir, Arc<App>, Arc<CfLoaderCatalog>) {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let loader = Arc::new(CfLoaderCatalog::default());
    let deps = AppDeps {
        catalog: loader.clone(),
        java: Arc::new(DummyJava),
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
    let http = Arc::new(HttpClient::new(NetConfig::new(dirs.cache())).unwrap());
    let hosts = HostAllowlist::new(&["127.0.0.1"]);
    let cf =
        CurseForge::with_base_url(Arc::clone(&http), "test-cf-key", server.uri()).with_hosts(hosts);
    let catalog = CurseforgeCatalog::new(http, "test-cf-key").with_curseforge(cf, hosts);
    let mut app = App::with_deps(dirs, deps)
        .unwrap()
        .with_curseforge(Arc::new(catalog));
    if with_key {
        app = app.with_curseforge_key(CurseforgeKey::parse("test-cf-key").unwrap());
    }
    (tmp, Arc::new(app), loader)
}

pub async fn finish(app: &App, job_id: u64) -> JobSnapshot {
    let mut rx = app.jobs().subscribe(job_id).unwrap();
    tokio::time::timeout(WAIT, rx.wait_for(|s| s.state != JobState::Running))
        .await
        .unwrap()
        .unwrap()
        .clone()
}

pub async fn mount_mod(server: &MockServer, id: u64, slug: &str, allow: bool, files: Vec<Value>) {
    Mock::given(method("GET"))
        .and(path(format!("/mods/{id}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "data": mod_json(id, slug, slug, 4471, allow) })),
        )
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/mods/{id}/files")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": files })))
        .mount(server)
        .await;
}

pub async fn mount_batch_files(server: &MockServer, map: HashMap<u64, Value>, mods: Vec<Value>) {
    Mock::given(method("POST"))
        .and(path("/mods/files"))
        .respond_with(move |req: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap();
            let data: Vec<Value> = body["fileIds"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|id| id.as_u64().and_then(|n| map.get(&n)).cloned())
                .collect();
            ResponseTemplate::new(200).set_body_json(json!({ "data": data }))
        })
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mods"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": mods })))
        .mount(server)
        .await;
}
