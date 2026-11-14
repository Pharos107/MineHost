//! Catalog và helper dựng `.mrpack` giả cho test route modpack.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use mh_app::content::{
    ContentCatalog, ContentFileInfo, ContentProject, ContentSearchResult, ContentVersion, PackFile,
    SearchFilters,
};
use mh_app::sources::{
    BoxFut, CancellationToken, CatalogVersion, InstallPlan, LoaderInstall, PlannedFile, ProgressFn,
    ResolvedServer, ServerJar,
};
use mh_app::{App, AppDeps, AppError, ServerCatalog};
use mh_core::instance::JarName;
use mh_core::{ErrorCode, Launch, LoaderKind, LoaderVersion, McVersion};
use mh_platform::DataDirs;
use serde_json::{Value, json};
use time::macros::datetime;

use super::super::{FakeJava, HOST, ORIGIN, PORT, TestApi, send};

struct PackServerCatalog;

impl ServerCatalog for PackServerCatalog {
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
        mc: &'a McVersion,
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
                    minecraft: mc.clone(),
                    loader_version: ver,
                    files: vec![PlannedFile {
                        url: "https://maven.fabricmc.net/fake.jar".to_owned(),
                        checksum: mh_app::sources::Checksum::sha256(&"0".repeat(64)).unwrap(),
                        size: Some(3),
                        dest: jar.to_owned(),
                        hosts: mh_app::sources::HostAllowlist::new(&[]),
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

struct PackModCatalog {
    versions: Vec<ContentVersion>,
    files: HashMap<String, Vec<u8>>,
}

impl PackModCatalog {
    fn sample() -> Self {
        let mod_url = "https://cdn.modrinth.com/data/fab/versions/vf1/fabric-api.jar";
        let pack_url = "https://cdn.modrinth.com/data/pack1/versions/vp1/pack.mrpack";
        let mod_bytes = store_zip(&[("fabric.mod.json", br#"{"id":"fab"}"#)]);
        let mrpack = sample_mrpack("API Pack", mod_url, mod_bytes.len() as u64);

        let mut files = HashMap::new();
        files.insert(mod_url.to_owned(), mod_bytes);
        files.insert(pack_url.to_owned(), mrpack.clone());

        let ver = ContentVersion {
            id: "vp1".to_owned(),
            project_id: "pack1".to_owned(),
            name: "v1.0.0".to_owned(),
            version_number: Some("1.0.0".to_owned()),
            version_type: "release".to_owned(),
            game_versions: vec!["1.21.4".to_owned()],
            loaders: vec!["fabric".to_owned()],
            date_published: datetime!(2025-01-10 0:00 UTC),
            files: vec![ContentFileInfo {
                url: Some(pack_url.to_owned()),
                filename: "pack.mrpack".to_owned(),
                size: mrpack.len() as u64,
                sha1: Some("0".repeat(40)),
                sha512: Some("0".repeat(128)),
                manual_download: false,
                download_page_url: None,
            }],
            dependencies: Vec::new(),
        };
        Self {
            versions: vec![ver],
            files,
        }
    }
}

impl ContentCatalog for PackModCatalog {
    fn search<'a>(
        &'a self,
        _: &'a str,
        _: &'a SearchFilters,
    ) -> BoxFut<'a, Result<ContentSearchResult, AppError>> {
        Box::pin(async { Err(AppError::new(ErrorCode::NetworkUnavailable, "unused")) })
    }

    fn project<'a>(&'a self, _: &'a str) -> BoxFut<'a, Result<ContentProject, AppError>> {
        Box::pin(async { Err(AppError::new(ErrorCode::NetworkUnavailable, "unused")) })
    }

    fn versions<'a>(
        &'a self,
        _: &'a str,
        _: LoaderKind,
        _: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<ContentVersion>, AppError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn version<'a>(&'a self, id: &'a str) -> BoxFut<'a, Result<ContentVersion, AppError>> {
        Box::pin(async move {
            self.versions
                .iter()
                .find(|v| v.id == id)
                .cloned()
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, id))
        })
    }

    fn download<'a>(
        &'a self,
        file: &'a ContentFileInfo,
        dest: &'a Path,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            let url = file
                .url
                .as_deref()
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, &file.filename))?;
            let bytes = self
                .files
                .get(url)
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, url))?;
            std::fs::write(dest, bytes).unwrap();
            Ok(())
        })
    }

    fn project_versions<'a>(
        &'a self,
        pid: &'a str,
    ) -> BoxFut<'a, Result<Vec<ContentVersion>, AppError>> {
        Box::pin(async move {
            Ok(self
                .versions
                .iter()
                .filter(|v| v.project_id == pid)
                .cloned()
                .collect())
        })
    }

    fn download_pack_file<'a>(
        &'a self,
        file: &'a PackFile,
        dest: &'a Path,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            let url = &file.urls[0];
            let bytes = self
                .files
                .get(url)
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, url))?;
            std::fs::write(dest, bytes).unwrap();
            Ok(())
        })
    }
}

pub(super) fn modpack_api() -> TestApi {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let deps = AppDeps {
        catalog: Arc::new(PackServerCatalog),
        java: Arc::new(FakeJava),
        port_free: Arc::new(|_| true),
        free_space: Arc::new(|_| Some(50 * 1024 * 1024 * 1024)),
        total_memory_mb: 16 * 1024,
        supervisor: mh_supervisor::SupervisorConfig::default(),
        save_flush_timeout: Duration::from_secs(60),
        metrics: mh_app::MetricsConfig::default(),
    };
    let app = Arc::new(
        App::with_deps(dirs, deps)
            .unwrap()
            .with_content(Arc::new(PackModCatalog::sample())),
    );
    let router = crate::router(Arc::clone(&app), PORT);
    TestApi {
        _tmp: tmp,
        app,
        router,
    }
}

pub(super) async fn wait_job_json(api: &TestApi, job_id: u64) -> Value {
    for _ in 0..200 {
        let (_, job) = api.get(&format!("/api/v1/jobs/{job_id}")).await;
        if job["state"] != "running" {
            assert_eq!(job["state"], "succeeded", "{job}");
            return job;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Job không xong");
}

pub(super) async fn upload_pack(
    router: &axum::Router,
    filename: &str,
    bytes: &[u8],
    fields: &[(&str, &str)],
) -> (StatusCode, Value) {
    let b = "PackBoundary";
    let mut body = Vec::new();
    for (k, v) in fields {
        body.extend(
            format!("--{b}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n")
                .into_bytes(),
        );
    }
    body.extend(
        format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\r\n"
        )
        .into_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend(format!("\r\n--{b}--\r\n").into_bytes());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/servers/modpack/upload")
        .header(header::HOST, HOST)
        .header(header::ORIGIN, ORIGIN)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={b}"),
        )
        .header(header::CONTENT_LENGTH, body.len().to_string())
        .body(Body::from(body))
        .unwrap();
    send(router.clone(), req).await
}

pub(super) fn sample_mrpack(name: &str, mod_url: &str, mod_len: u64) -> Vec<u8> {
    let index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0.0",
        "name": name,
        "dependencies": { "minecraft": "1.21.4", "fabric-loader": "0.16.9" },
        "files": [
            {
                "path": "mods/fabric-api.jar",
                "hashes": { "sha1": "0".repeat(40), "sha512": "0".repeat(128) },
                "downloads": [mod_url],
                "fileSize": mod_len,
                "env": { "client": "required", "server": "required" }
            },
            {
                "path": "mods/cli.jar",
                "hashes": { "sha1": "0".repeat(40), "sha512": "0".repeat(128) },
                "downloads": ["https://cdn.modrinth.com/cli.jar"],
                "fileSize": 10,
                "env": { "client": "required", "server": "unsupported" }
            }
        ]
    });
    store_zip(&[
        ("modrinth.index.json", index.to_string().as_bytes()),
        ("overrides/config/api.txt", b"ok"),
    ])
}

fn store_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut dir = Vec::new();
    for &(name, data) in entries {
        let off = out.len() as u32;
        let crc = crc32(data);
        let n = name.as_bytes();
        let (nl, dl) = (n.len() as u16, data.len() as u32);
        out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&dl.to_le_bytes());
        out.extend_from_slice(&dl.to_le_bytes());
        out.extend_from_slice(&nl.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(n);
        out.extend_from_slice(data);
        dir.push((off, crc, dl, n));
    }
    let cd_off = out.len() as u32;
    for (off, crc, dl, n) in &dir {
        out.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&dl.to_le_bytes());
        out.extend_from_slice(&dl.to_le_bytes());
        out.extend_from_slice(&(n.len() as u16).to_le_bytes());
        out.extend_from_slice(&[0; 12]);
        out.extend_from_slice(&off.to_le_bytes());
        out.extend_from_slice(n);
    }
    let cd_size = out.len() as u32 - cd_off;
    let cnt = dir.len() as u16;
    out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
    out.extend_from_slice(&cnt.to_le_bytes());
    out.extend_from_slice(&cnt.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}
