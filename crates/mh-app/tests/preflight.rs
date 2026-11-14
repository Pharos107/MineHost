//! Kiểm tra trước khi bật server (`App::preflight`, `start_with`, `preflight_blocking`, P3 PR-21).

#![allow(clippy::unwrap_used)]

mod content_support;

use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use content_support::{WAIT, make_server, test_app};
use mh_app::content::{ContentCatalog, NoContentCatalog};
use mh_app::{App, AppEvent};
use mh_core::{ErrorCode, LoaderKind, ModMetaFormat, ServerId, ServerState};
use mh_store::{CONTENT_SCHEMA, ContentManifest, PendingContent, ServerStore};
use zip::write::SimpleFileOptions;

fn server_paths(app: &App, id: ServerId) -> mh_store::ServerPaths {
    ServerStore::new(app.dirs().servers(), app.dirs().trash()).paths(id)
}

fn prepare_runnable(app: &App, id: ServerId) {
    let paths = server_paths(app, id);
    fs::create_dir_all(&paths.server_dir).unwrap();
    fs::write(paths.server_dir.join("server.jar"), b"fake jar").unwrap();
    app.accept_eula(id).unwrap();
}

fn write_jar(path: &Path, entries: &[(&str, &[u8])]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let cur = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cur);
    let opts = SimpleFileOptions::default();
    for (name, data) in entries {
        zip.start_file(*name, opts).unwrap();
        zip.write_all(data).unwrap();
    }
    let bytes = zip.finish().unwrap().into_inner();
    fs::write(path, bytes).unwrap();
}

async fn wait_state(app: &App, id: ServerId, target: ServerState) {
    let mut rx = app.subscribe_events();
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        if let Ok(view) = app.get(id)
            && view.state == target
        {
            return;
        }
        let now = tokio::time::Instant::now();
        assert!(now < deadline, "hết giờ chờ state {target:?}");
        let step = (deadline - now).min(Duration::from_millis(200));
        let _ = tokio::time::timeout(step, async {
            while let Ok(ev) = rx.recv().await {
                if matches!(ev, AppEvent::ServerChanged(sid) if sid == id) {
                    break;
                }
            }
        })
        .await;
    }
}

#[tokio::test]
async fn missing_dep_blocks_start_and_ignore_preflight_allows_start() {
    let (_tmp, app) = test_app(Arc::new(NoContentCatalog) as Arc<dyn ContentCatalog>);
    let id = make_server(&app, LoaderKind::Fabric);
    prepare_runnable(&app, id);
    let mods = server_paths(&app, id).server_dir.join("mods");

    write_jar(
        &mods.join("mod-a.jar"),
        &[(
            "fabric.mod.json",
            br#"{"schemaVersion":1,"id":"mod-a","version":"1.0.0","name":"Mod A","depends":{"missing-lib":">=1.0.0","minecraft":">=1.21","fabricloader":">=0.15.0"}}"#,
        )],
    );

    let report = app.preflight(id).await.unwrap();
    assert_eq!(report.missing_deps.len(), 1);
    assert_eq!(report.missing_deps[0].id, "missing-lib");
    assert_eq!(report.missing_deps[0].version_req, ">=1.0.0");
    assert_eq!(report.missing_deps[0].required_by, "Mod A");
    assert!(report.is_blocking());
    assert!(app.get(id).unwrap().preflight_blocking);

    let err = app.start(id).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::DependencyMissing);
    assert_eq!(err.params["missing"][0]["id"], "missing-lib");

    // Mod đang tắt không bị tính vào preflight.
    app.set_content_enabled(id, "mod-a.jar", false)
        .await
        .unwrap();
    assert!(!app.preflight(id).await.unwrap().is_blocking());
    assert!(!app.get(id).unwrap().preflight_blocking);
    app.set_content_enabled(id, "mod-a.jar", true)
        .await
        .unwrap();
    assert!(app.get(id).unwrap().preflight_blocking);

    // Bật với ignore_preflight = true đi tới Đang chạy.
    app.start_with(id, true).await.unwrap();
    wait_state(&app, id, ServerState::Running).await;
    app.stop(id).await.unwrap();
    wait_state(&app, id, ServerState::Stopped).await;
}

#[tokio::test]
async fn version_mismatch_and_unparsed_req_warn_without_blocking() {
    let (_tmp, app) = test_app(Arc::new(NoContentCatalog) as Arc<dyn ContentCatalog>);
    let id = make_server(&app, LoaderKind::Fabric);
    prepare_runnable(&app, id);
    let mods = server_paths(&app, id).server_dir.join("mods");

    write_jar(
        &mods.join("lib-a.jar"),
        &[("fabric.mod.json", br#"{"id":"lib-a","version":"1.0.0"}"#)],
    );
    write_jar(
        &mods.join("lib-b.jar"),
        &[(
            "fabric.mod.json",
            br#"{"id":"lib-b","version":"custom+build.xyz"}"#,
        )],
    );
    write_jar(
        &mods.join("consumer.jar"),
        &[(
            "fabric.mod.json",
            br#"{"id":"consumer","name":"Consumer","version":"1.0.0","depends":{"lib-a":">=2.0.0","lib-b":">=9.9.9"}}"#,
        )],
    );

    let report = app.preflight(id).await.unwrap();
    assert!(report.missing_deps.is_empty());
    assert!(!report.is_blocking());
    assert!(!app.get(id).unwrap().preflight_blocking);
    assert_eq!(report.version_mismatches.len(), 1);
    assert_eq!(report.version_mismatches[0].id, "lib-a");
    assert_eq!(report.version_mismatches[0].installed_version, "1.0.0");
    assert_eq!(report.version_mismatches[0].version_req, ">=2.0.0");
    assert_eq!(report.version_mismatches[0].required_by, "Consumer");

    app.start(id).await.unwrap();
    wait_state(&app, id, ServerState::Running).await;
    app.stop(id).await.unwrap();
    wait_state(&app, id, ServerState::Stopped).await;
}

#[tokio::test]
async fn detects_wrong_loader_and_client_only_respecting_quilt_and_multiloader() {
    let (_tmp, app) = test_app(Arc::new(NoContentCatalog) as Arc<dyn ContentCatalog>);

    // Forge: jar chỉ có fabric.mod.json + client side -> báo cả wrong_loader lẫn client_only;
    // jar có cả fabric.mod.json và META-INF/mods.toml không bị coi sai loader.
    let forge = make_server(&app, LoaderKind::Forge);
    let forge_mods = server_paths(&app, forge).server_dir.join("mods");
    write_jar(
        &forge_mods.join("fabric-only.jar"),
        &[(
            "fabric.mod.json",
            br#"{"id":"sodium","name":"Sodium","version":"0.6.0","environment":"client"}"#,
        )],
    );
    write_jar(
        &forge_mods.join("multi.jar"),
        &[
            (
                "fabric.mod.json",
                br#"{"id":"multi","name":"MultiMod","version":"1.0.0"}"#,
            ),
            (
                "META-INF/mods.toml",
                b"[[mods]]\nmodId=\"multi\"\ndisplayName=\"MultiMod\"\nversion=\"1.0.0\"\n",
            ),
        ],
    );

    let forge_report = app.preflight(forge).await.unwrap();
    assert_eq!(forge_report.wrong_loader.len(), 1);
    assert_eq!(forge_report.wrong_loader[0].file_name, "fabric-only.jar");
    assert_eq!(
        forge_report.wrong_loader[0].formats,
        vec![ModMetaFormat::FabricModJson]
    );
    assert_eq!(forge_report.client_only.len(), 1);
    assert_eq!(forge_report.client_only[0].file_name, "fabric-only.jar");

    // Quilt chạy được mod Fabric, nhưng cảnh báo mod chỉ có META-INF/mods.toml.
    let quilt = make_server(&app, LoaderKind::Quilt);
    let quilt_mods = server_paths(&app, quilt).server_dir.join("mods");
    write_jar(
        &quilt_mods.join("fabric-ok.jar"),
        &[(
            "fabric.mod.json",
            br#"{"id":"fabric-ok","name":"Fabric OK","version":"1.0.0"}"#,
        )],
    );
    write_jar(
        &quilt_mods.join("forge-bad.jar"),
        &[(
            "META-INF/mods.toml",
            b"[[mods]]\nmodId=\"forgebad\"\ndisplayName=\"Forge Bad\"\nversion=\"1.0.0\"\n",
        )],
    );

    let quilt_report = app.preflight(quilt).await.unwrap();
    assert_eq!(quilt_report.wrong_loader.len(), 1);
    assert_eq!(quilt_report.wrong_loader[0].file_name, "forge-bad.jar");
}

#[tokio::test]
async fn pending_blocks_start_and_vanilla_skips_preflight() {
    let (_tmp, app) = test_app(Arc::new(NoContentCatalog) as Arc<dyn ContentCatalog>);
    let fabric = make_server(&app, LoaderKind::Fabric);
    prepare_runnable(&app, fabric);
    let paths = server_paths(&app, fabric);
    let manifest = ContentManifest {
        schema: CONTENT_SCHEMA,
        file: vec![],
        pending: vec![PendingContent {
            path: "mods/manual-cf.jar".to_owned(),
            project_id: 111,
            file_id: 222,
            sha1: "abc".to_owned(),
            page_url: "https://example.invalid/mod".to_owned(),
            extra: toml::Table::new(),
        }],
        extra: toml::Table::new(),
    };
    manifest.save(&paths.content_file).unwrap();

    let report = app.preflight(fabric).await.unwrap();
    assert_eq!(report.pending.len(), 1);
    assert!(report.is_blocking());
    assert!(app.get(fabric).unwrap().preflight_blocking);

    let err = app.start(fabric).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::CurseforgeFilesPending);

    app.start_with(fabric, true).await.unwrap();
    wait_state(&app, fabric, ServerState::Running).await;
    app.stop(fabric).await.unwrap();
    wait_state(&app, fabric, ServerState::Stopped).await;

    // Vanilla bỏ qua preflight kể cả khi có thư mục mods/ hoặc pending.
    let vanilla = make_server(&app, LoaderKind::Vanilla);
    prepare_runnable(&app, vanilla);
    let vpaths = server_paths(&app, vanilla);
    write_jar(
        &vpaths.server_dir.join("mods/broken.jar"),
        &[(
            "fabric.mod.json",
            br#"{"id":"broken","depends":{"absent":"*"}}"#,
        )],
    );
    manifest.save(&vpaths.content_file).unwrap();

    assert!(!app.preflight(vanilla).await.unwrap().is_blocking());
    assert!(!app.get(vanilla).unwrap().preflight_blocking);
    app.start(vanilla).await.unwrap();
    wait_state(&app, vanilla, ServerState::Running).await;
    app.stop(vanilla).await.unwrap();
    wait_state(&app, vanilla, ServerState::Stopped).await;
}
