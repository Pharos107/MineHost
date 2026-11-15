//! Tích hợp luồng CurseForge trong `mh-app`: dependency bắt buộc, file tải tay `[[pending]]` và key.

#![allow(clippy::unwrap_used, clippy::panic)]

mod content_support;

use std::sync::Arc;
use std::time::Duration;

use content_support::{FakeJava, FakeServerCatalog, build_jar, finish_job, make_server};
use mh_app::content::{CatalogSource, CurseForge, CurseforgeCatalog, ProjectType};
use mh_app::{App, AppDeps, JobState, MetricsConfig};
use mh_core::{CurseforgeKey, ErrorCode, LoaderKind};
use mh_net::{HostAllowlist, HttpClient, NetConfig};
use mh_platform::DataDirs;
use mh_store::{ContentManifest, ContentSource, ProjectId, ServerStore};
use mh_supervisor::SupervisorConfig;
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn mod_json(id: u64, slug: &str, name: &str, allow: bool) -> Value {
    json!({
        "id": id,
        "name": name,
        "slug": slug,
        "summary": format!("Mô tả {name}"),
        "classId": 6,
        "downloadCount": 1000,
        "allowModDistribution": allow,
        "logo": null,
        "links": {
            "websiteUrl": format!("https://www.curseforge.com/minecraft/mc-mods/{slug}")
        }
    })
}

fn file_json(
    id: u64,
    mod_id: u64,
    file_name: &str,
    download_url: Option<String>,
    sha1: &str,
    size: usize,
    deps: Value,
) -> Value {
    json!({
        "id": id,
        "modId": mod_id,
        "displayName": file_name,
        "fileName": file_name,
        "releaseType": 1,
        "fileDate": "2026-11-01T00:00:00Z",
        "fileLength": size,
        "downloadUrl": download_url,
        "hashes": [{ "algo": 1, "value": sha1 }],
        "gameVersions": ["1.21.4", "Fabric"],
        "dependencies": deps,
        "isServerPack": false
    })
}

fn cf_app(server: &MockServer, with_key: bool) -> (tempfile::TempDir, Arc<App>) {
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
    (tmp, Arc::new(app))
}

#[tokio::test]
async fn installs_curseforge_required_dep_recursively_and_records_content_toml() {
    let server = MockServer::start().await;
    let base = server.uri();

    let fapi_jar = build_jar(Some("*"), "fapi");
    let jei_jar = build_jar(Some("*"), "jei");
    let client_jar = build_jar(Some("client"), "client-cf");

    // 238222 (JEI) --relationType=3--> 306612 (Fabric API) --vòng relationType=3--> 238222
    //                                                    \--relationType=2--> 999 (optional, bỏ qua)
    Mock::given(method("GET"))
        .and(path("/mods/238222"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "data": mod_json(238222, "jei", "JEI", true) })),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mods/306612"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                json!({ "data": mod_json(306612, "fabric-api", "Fabric API", true) }),
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mods/888001"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "data": mod_json(888001, "iris-cf", "Iris CF", true) })),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/mods/238222/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [file_json(
                5800001,
                238222,
                "jei-19.jar",
                Some(format!("{base}/cdn/jei.jar")),
                &sha1_hex(&jei_jar),
                jei_jar.len(),
                json!([{ "modId": 306612, "relationType": 3 }]),
            )]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mods/306612/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [file_json(
                5800000,
                306612,
                "fapi-0.110.jar",
                Some(format!("{base}/cdn/fapi.jar")),
                &sha1_hex(&fapi_jar),
                fapi_jar.len(),
                json!([
                    { "modId": 238222, "relationType": 3 },
                    { "modId": 999, "relationType": 2 }
                ]),
            )]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mods/888001/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [file_json(
                5800888,
                888001,
                "iris-cf.jar",
                Some(format!("{base}/cdn/iris.jar")),
                &sha1_hex(&client_jar),
                client_jar.len(),
                json!([]),
            )]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/cdn/jei.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(jei_jar.clone()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/cdn/fapi.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(fapi_jar.clone()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/cdn/iris.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(client_jar.clone()))
        .mount(&server)
        .await;

    let (_tmp, app) = cf_app(&server, true);
    let id = make_server(&app, LoaderKind::Fabric);

    let plan = app
        .plan_content_install(id, CatalogSource::Curseforge, "238222", None)
        .await
        .unwrap();
    let pids: Vec<_> = plan.items.iter().map(|i| i.project_id.as_str()).collect();
    assert_eq!(pids, ["238222", "306612"]);
    assert!(!plan.items[0].is_dependency);
    assert!(plan.items[1].is_dependency);
    assert!(!plan.items[0].file.manual_download);

    let job = app
        .install_content(id, CatalogSource::Curseforge, "238222", None)
        .unwrap();
    assert_eq!(finish_job(&app, job).await.state, JobState::Succeeded);

    let paths = ServerStore::new(app.dirs().servers(), app.dirs().trash()).paths(id);
    let manifest = ContentManifest::load(&paths.content_file).unwrap();
    assert_eq!(manifest.file.len(), 2);
    assert!(
        manifest
            .file
            .iter()
            .all(|r| r.source == ContentSource::Curseforge)
    );
    assert_eq!(manifest.file[0].project_id, Some(ProjectId::Int(238222)));
    assert_eq!(manifest.file[0].version_id.as_deref(), Some("5800001"));
    assert_eq!(manifest.file[1].project_id, Some(ProjectId::Int(306612)));

    // Cài mod chỉ chạy client từ CurseForge: phải dựa metadata jar để đưa vào `mods-disabled/`.
    let cjob = app
        .install_content(id, CatalogSource::Curseforge, "888001", None)
        .unwrap();
    assert_eq!(finish_job(&app, cjob).await.state, JobState::Succeeded);
    assert!(paths.server_dir.join("mods-disabled/iris-cf.jar").is_file());
    let list = app.content(id).await.unwrap();
    assert!(list.files.iter().any(|f| f.file_name == "iris-cf.jar"
        && !f.enabled
        && f.record.as_ref().is_some_and(|r| r.client_only)));
}

#[tokio::test]
async fn curseforge_restricted_mod_installs_deps_and_writes_pending() {
    let server = MockServer::start().await;
    let base = server.uri();
    let fapi_jar = build_jar(Some("*"), "fapi");
    let restricted_sha1 = sha1_hex(b"restricted-placeholder");

    Mock::given(method("GET"))
        .and(path("/mods/777001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": mod_json(777001, "restricted-mod", "Restricted Mod", false)
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mods/306612"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": mod_json(306612, "fabric-api", "Fabric API", true)
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mods/777001/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [file_json(
                7770001,
                777001,
                "restricted-1.0.jar",
                None,
                &restricted_sha1,
                2048,
                json!([{ "modId": 306612, "relationType": 3 }]),
            )]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mods/306612/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [file_json(
                5800000,
                306612,
                "fapi-0.110.jar",
                Some(format!("{base}/cdn/fapi.jar")),
                &sha1_hex(&fapi_jar),
                fapi_jar.len(),
                json!([]),
            )]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/cdn/fapi.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(fapi_jar))
        .mount(&server)
        .await;

    let (_tmp, app) = cf_app(&server, true);
    let id = make_server(&app, LoaderKind::Fabric);

    let plan = app
        .plan_content_install(id, CatalogSource::Curseforge, "777001", None)
        .await
        .unwrap();
    assert_eq!(plan.items.len(), 2);
    assert!(plan.items[0].file.manual_download);
    assert_eq!(
        plan.items[0].file.download_page_url.as_deref(),
        Some("https://www.curseforge.com/minecraft/mc-mods/restricted-mod/download/7770001")
    );
    assert!(!plan.items[1].file.manual_download);

    let job = app
        .install_content(id, CatalogSource::Curseforge, "777001", None)
        .unwrap();
    assert_eq!(finish_job(&app, job).await.state, JobState::Succeeded);

    let list = app.content(id).await.unwrap();
    assert_eq!(list.files.len(), 1);
    assert_eq!(list.files[0].file_name, "fapi-0.110.jar");
    assert_eq!(list.pending.len(), 1);
    assert_eq!(list.pending[0].path, "mods/restricted-1.0.jar");
    assert_eq!(list.pending[0].project_id, 777001);
    assert_eq!(list.pending[0].file_id, 7770001);
    assert_eq!(list.pending[0].sha1, restricted_sha1);
    assert_eq!(
        list.pending[0].page_url,
        "https://www.curseforge.com/minecraft/mc-mods/restricted-mod/download/7770001"
    );
}

#[tokio::test]
async fn curseforge_missing_or_rejected_key_returns_unavailable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/mods/search"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let (_tmp1, no_key_app) = cf_app(&server, false);
    let id1 = make_server(&no_key_app, LoaderKind::Fabric);
    assert_eq!(
        no_key_app
            .search_content(CatalogSource::Curseforge, "jei", None, Some(id1), 1)
            .await
            .unwrap_err()
            .code,
        ErrorCode::CurseforgeUnavailable
    );
    assert_eq!(
        no_key_app
            .install_content(id1, CatalogSource::Curseforge, "238222", None)
            .unwrap_err()
            .code,
        ErrorCode::CurseforgeUnavailable
    );

    let (_tmp2, bad_key_app) = cf_app(&server, true);
    let id2 = make_server(&bad_key_app, LoaderKind::Fabric);
    assert_eq!(
        bad_key_app
            .search_content(CatalogSource::Curseforge, "jei", None, Some(id2), 1)
            .await
            .unwrap_err()
            .code,
        ErrorCode::CurseforgeUnavailable
    );

    // Paper không dùng CurseForge trong P3: tìm kiếm rỗng, plan báo incompatible.
    let paper = make_server(&bad_key_app, LoaderKind::Paper);
    assert!(
        bad_key_app
            .search_content(
                CatalogSource::Curseforge,
                "",
                Some(ProjectType::Plugin),
                Some(paper),
                1
            )
            .await
            .unwrap()
            .hits
            .is_empty()
    );
    Mock::given(method("GET"))
        .and(path("/mods/238222"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "data": mod_json(238222, "jei", "JEI", true) })),
        )
        .mount(&server)
        .await;
    assert_eq!(
        bad_key_app
            .plan_content_install(paper, CatalogSource::Curseforge, "238222", None)
            .await
            .unwrap_err()
            .code,
        ErrorCode::ContentIncompatible
    );
}
