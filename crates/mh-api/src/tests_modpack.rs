//! Kiểm thử route tạo server từ modpack (`/servers/modpack` và `/servers/modpack/upload`).

#[path = "tests_modpack_support.rs"]
mod support;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use mh_app::content::{
    BlockedPackFile, CfResolvedFiles, ContentCatalog, ContentFileInfo, ContentProject,
    ContentSearchResult, ContentVersion, PackFile, SearchFilters,
};
use mh_app::sources::{BoxFut, CancellationToken, Checksum, HostAllowlist, ProgressFn};
use mh_app::{App, AppError, ModpackSummary};
use mh_core::{CfManifestFile, CurseforgeKey, ErrorCode, LoaderKind, McVersion};
use mh_platform::DataDirs;
use serde_json::json;
use time::macros::datetime;

use super::{HOST, ORIGIN, PORT, TestApi, raw, send};
use support::{
    modpack_api, pack_deps, sample_cf_zip, sample_mrpack, store_zip, upload_pack, wait_job_json,
};

#[tokio::test]
async fn post_servers_modpack_creates_job_and_returns_summary() {
    let api = modpack_api();
    let (status, created) = api
        .post(
            "/api/v1/servers/modpack",
            Some(json!({ "source": "modrinth", "project_id": "pack1", "name": "Pack API", "port": 25565 })),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{created}");
    let job = wait_job_json(&api, created["job_id"].as_u64().unwrap()).await;
    assert_eq!(job["kind"], "create_modpack");
    let summary: ModpackSummary = serde_json::from_value(job["result"].clone()).unwrap();
    assert_eq!(summary.installed_mods, 1);
    assert_eq!(summary.skipped_client, ["mods/cli.jar"]);
    assert!(summary.pending_curseforge.is_empty());

    let sid = created["server_id"].as_str().unwrap();
    let (status, srv) = api.get(&format!("/api/v1/servers/{sid}")).await;
    assert_eq!(status, StatusCode::OK, "{srv}");
    assert_eq!(srv["loader"], "fabric");
}

#[tokio::test]
async fn post_servers_modpack_validates_inputs() {
    let api = modpack_api();
    let (st, err) = api
        .post(
            "/api/v1/servers/modpack",
            Some(json!({ "source": "curseforge", "project_id": "x", "name": "CF" })),
        )
        .await;
    assert_eq!(
        (st, err["code"].as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Some("CURSEFORGE_UNAVAILABLE")
        )
    );

    for body in [
        json!({ "source": "other", "project_id": "x", "name": "Bad" }),
        json!({ "source": "modrinth", "project_id": "   ", "name": "Bad" }),
    ] {
        let (st, err) = api.post("/api/v1/servers/modpack", Some(body)).await;
        assert_eq!(
            (st, err["code"].as_str()),
            (StatusCode::BAD_REQUEST, Some("INVALID_INPUT"))
        );
    }
}

#[tokio::test]
async fn post_servers_modpack_upload_mrpack_and_reject_bad_ext() {
    let api = modpack_api();
    let mrpack = sample_mrpack(
        "Upload API",
        "https://cdn.modrinth.com/data/fab/versions/vf1/fabric-api.jar",
        38,
    );
    let (st, created) = upload_pack(
        &api.router,
        "pack.mrpack",
        &mrpack,
        &[
            ("name", "Upload API"),
            ("memory_mb", r#"{"min":2048,"max":6144}"#),
            ("port", "25566"),
        ],
    )
    .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{created}");
    let job = wait_job_json(&api, created["job_id"].as_u64().unwrap()).await;
    assert_eq!(job["result"]["installed_mods"], 1);
    let (_, srv) = api
        .get(&format!(
            "/api/v1/servers/{}",
            created["server_id"].as_str().unwrap()
        ))
        .await;
    assert_eq!(srv["port"], 25566);

    let (st, err) = upload_pack(&api.router, "bad.tar", b"tarbytes", &[("name", "Bad Tar")]).await;
    assert_eq!(
        (st, err["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("MODPACK_INVALID"))
    );
    assert!(
        err["params"]["reason"]
            .as_str()
            .unwrap()
            .contains(".mrpack")
    );

    let (st, err) = upload_pack(&api.router, "pack.mrpack", &mrpack, &[]).await;
    assert_eq!(
        (st, err["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("INVALID_INPUT"))
    );
}

#[tokio::test]
async fn post_servers_modpack_curseforge_checks_key_and_uploads_zip() {
    let no_key = curseforge_modpack_api(false);
    let (st, err) = no_key
        .post(
            "/api/v1/servers/modpack",
            Some(json!({
                "source": "curseforge",
                "project_id": "910001",
                "version_id": "5900001",
                "name": "Thiếu Key"
            })),
        )
        .await;
    assert_eq!(
        (st, err["code"].as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Some("CURSEFORGE_UNAVAILABLE")
        )
    );

    let api = curseforge_modpack_api(true);
    let (st, created) = api
        .post(
            "/api/v1/servers/modpack",
            Some(json!({
                "source": "curseforge",
                "project_id": "910001",
                "version_id": "5900001",
                "name": "CF API"
            })),
        )
        .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{created}");
    let job = wait_job_json(&api, created["job_id"].as_u64().unwrap()).await;
    assert_eq!(
        job["result"]["pending_curseforge"],
        json!(["mods/restricted-1.0.jar"])
    );

    let zip_bytes = sample_cf_zip();
    let (st, up) = upload_pack(
        &api.router,
        "pack.zip",
        &zip_bytes,
        &[("name", "CF Zip"), ("port", "25568")],
    )
    .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{up}");
    let job_up = wait_job_json(&api, up["job_id"].as_u64().unwrap()).await;
    assert_eq!(
        job_up["result"]["pending_curseforge"],
        json!(["mods/restricted-1.0.jar"])
    );
}

fn curseforge_modpack_api(with_key: bool) -> TestApi {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let mut app = App::with_deps(dirs, pack_deps())
        .unwrap()
        .with_curseforge(Arc::new(CfPackStubCatalog::sample()));
    if with_key {
        app = app.with_curseforge_key(CurseforgeKey::parse("test-cf-key").unwrap());
    }
    let app = Arc::new(app);
    let router = crate::router(Arc::clone(&app), PORT);
    TestApi {
        _tmp: tmp,
        app,
        router,
    }
}

struct CfPackStubCatalog {
    ver: ContentVersion,
    files: HashMap<String, Vec<u8>>,
    mod_url: String,
}

impl CfPackStubCatalog {
    fn sample() -> Self {
        let pack_url = "https://edge.forgecdn.net/files/5900/1/pack.zip".to_owned();
        let mod_url = "https://edge.forgecdn.net/files/5800/0/fabric-api.jar".to_owned();
        let pack_bytes = sample_cf_zip();
        let mod_bytes = store_zip(&[("fabric.mod.json", br#"{"id":"fapi","environment":"*"}"#)]);
        let ver = ContentVersion {
            id: "5900001".to_owned(),
            project_id: "910001".to_owned(),
            name: "CF API Pack 1.0".to_owned(),
            version_number: None,
            version_type: "release".to_owned(),
            game_versions: vec!["1.21.4".to_owned()],
            loaders: vec!["fabric".to_owned()],
            date_published: datetime!(2025-01-10 0:00 UTC),
            files: vec![ContentFileInfo {
                url: Some(pack_url.clone()),
                filename: "pack.zip".to_owned(),
                size: pack_bytes.len() as u64,
                sha1: Some("a".repeat(40)),
                sha512: None,
                manual_download: false,
                download_page_url: None,
            }],
            dependencies: Vec::new(),
        };
        Self {
            ver,
            files: HashMap::from([(pack_url, pack_bytes), (mod_url.clone(), mod_bytes)]),
            mod_url,
        }
    }
}

impl ContentCatalog for CfPackStubCatalog {
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
            (id == self.ver.id)
                .then(|| self.ver.clone())
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
            Ok(if pid == self.ver.project_id {
                vec![self.ver.clone()]
            } else {
                Vec::new()
            })
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
            let bytes = self
                .files
                .get(&file.urls[0])
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, &file.path))?;
            std::fs::write(dest, bytes).unwrap();
            Ok(())
        })
    }

    fn resolve_curseforge_files<'a>(
        &'a self,
        _: &'a [CfManifestFile],
    ) -> BoxFut<'a, Result<CfResolvedFiles, AppError>> {
        Box::pin(async move {
            let mod_len = self.files.get(&self.mod_url).map_or(0, Vec::len) as u64;
            let pack = vec![PackFile {
                path: "mods/fabric-api.jar".to_owned(),
                urls: vec![self.mod_url.clone()],
                checksum: Checksum::sha1(&"a".repeat(40)).unwrap(),
                sha1: "a".repeat(40),
                size: mod_len,
                hosts: HostAllowlist::new(&[]),
                cf_ids: Some((306612, 5800000)),
            }];
            let blocked = vec![BlockedPackFile {
                path: "mods/restricted-1.0.jar".to_owned(),
                project_id: 999001,
                file_id: 5800999,
                sha1: "b".repeat(40),
                page_url: "https://www.curseforge.com/minecraft/mc-mods/r/download/5800999"
                    .to_owned(),
            }];
            Ok((pack, blocked))
        })
    }
}

#[tokio::test]
async fn modpack_upload_guard_and_openapi_routes() {
    let api = modpack_api();
    for bad_path in [
        "/api/v1/servers/modpack",
        "/api/v1/servers/modpack/upload/extra",
    ] {
        let req = raw(Method::POST, bad_path)
            .header(header::HOST, HOST)
            .header(header::ORIGIN, ORIGIN)
            .header(header::CONTENT_TYPE, "multipart/form-data; boundary=b")
            .header(header::CONTENT_LENGTH, "1")
            .body(Body::from("x"))
            .unwrap();
        let (st, err) = send(api.router.clone(), req).await;
        assert_eq!(
            (st, err["code"].as_str()),
            (StatusCode::UNSUPPORTED_MEDIA_TYPE, Some("INVALID_INPUT")),
            "{bad_path}"
        );
    }

    let bare = raw(Method::POST, "/api/v1/servers/modpack/upload")
        .header(header::HOST, HOST)
        .header(header::CONTENT_TYPE, "multipart/form-data; boundary=b");
    let (st, _) = send(api.router.clone(), bare.body(Body::empty()).unwrap()).await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    let spec = crate::openapi();
    for path in ["/api/v1/servers/modpack", "/api/v1/servers/modpack/upload"] {
        assert!(spec.paths.paths.contains_key(path), "thiếu {path}");
    }
}
