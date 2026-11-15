//! Tải tay file CurseForge đang chờ: mod chỉ client vào thư mục tắt, giữ dấu modpack, khớp theo
//! tên khi CurseForge không có SHA-1.

#![allow(clippy::unwrap_used, clippy::panic)]

mod content_support;

use std::sync::Arc;

use content_support::{FakeModCatalog, build_jar, make_server, test_app};
use mh_core::{ErrorCode, LoaderKind};
use mh_store::{ContentManifest, ContentSource, PendingContent, ServerStore};

fn pending(file: &str, sha1: String, from_modpack: bool) -> PendingContent {
    PendingContent {
        path: format!("mods/{file}"),
        name: None,
        project_id: 42,
        file_id: 4242,
        sha1,
        page_url: "https://example.invalid".to_owned(),
        from_modpack,
        client_only: false,
        extra: Default::default(),
    }
}

#[tokio::test]
async fn client_only_pack_file_goes_to_disabled_and_stays_modpack() {
    let (tmp, app) = test_app(Arc::new(FakeModCatalog::default()));
    let id = make_server(&app, LoaderKind::Fabric);
    let paths = ServerStore::new(app.dirs().servers(), app.dirs().trash()).paths(id);
    let src = tmp.path().join("download.jar");
    std::fs::write(&src, build_jar(Some("client"), "CLIENT")).unwrap();
    let sha1 = mh_store::inspect_jar_file(&src).unwrap().sha1;
    let mut manifest = ContentManifest::load(&paths.content_file).unwrap();
    manifest.pending.push(pending("zoom.jar", sha1, true));
    manifest.save(&paths.content_file).unwrap();

    let file = app
        .upload_pending_content(id, "download.jar", &src)
        .await
        .unwrap();
    assert_eq!(file.path, "mods-disabled/zoom.jar");
    assert!(!file.enabled);
    assert!(paths.server_dir.join("mods-disabled/zoom.jar").is_file());
    assert!(!paths.server_dir.join("mods/zoom.jar").exists());
    let rec = file.record.unwrap();
    assert_eq!(rec.source, ContentSource::Modpack);
    assert!(rec.from_modpack);
    assert!(rec.client_only);
}

#[tokio::test]
async fn pending_without_sha1_matches_by_file_name() {
    let (tmp, app) = test_app(Arc::new(FakeModCatalog::default()));
    let id = make_server(&app, LoaderKind::Fabric);
    let paths = ServerStore::new(app.dirs().servers(), app.dirs().trash()).paths(id);
    let mut manifest = ContentManifest::load(&paths.content_file).unwrap();
    manifest
        .pending
        .push(pending("nohash.jar", String::new(), false));
    manifest.save(&paths.content_file).unwrap();

    let other = tmp.path().join("other.jar");
    std::fs::write(&other, build_jar(None, "OTHER")).unwrap();
    let err = app
        .upload_pending_content(id, "other.jar", &other)
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::UploadHashMismatch);

    let src = tmp.path().join("nohash.jar");
    std::fs::write(&src, build_jar(None, "NOHASH")).unwrap();
    let file = app
        .upload_pending_content(id, "nohash.jar", &src)
        .await
        .unwrap();
    assert_eq!(file.path, "mods/nohash.jar");
    assert!(file.enabled);
    assert_eq!(file.record.unwrap().source, ContentSource::Curseforge);
    assert!(app.content(id).await.unwrap().pending.is_empty());
}

#[tokio::test]
async fn pending_marked_client_only_by_modrinth_goes_to_disabled() {
    // Jar không tự khai báo chỉ chạy client; lúc cài pack Modrinth đã báo vậy (spec P3, mục 3.5).
    let (tmp, app) = test_app(Arc::new(FakeModCatalog::default()));
    let id = make_server(&app, LoaderKind::Forge);
    let paths = ServerStore::new(app.dirs().servers(), app.dirs().trash()).paths(id);
    let src = tmp.path().join("watermedia.jar");
    std::fs::write(&src, build_jar(None, "WATERMEDIA")).unwrap();
    let sha1 = mh_store::inspect_jar_file(&src).unwrap().sha1;
    let mut manifest = ContentManifest::load(&paths.content_file).unwrap();
    let mut entry = pending("watermedia.jar", sha1, true);
    entry.client_only = true;
    manifest.pending.push(entry);
    manifest.save(&paths.content_file).unwrap();

    let file = app
        .upload_pending_content(id, "watermedia.jar", &src)
        .await
        .unwrap();
    assert_eq!(file.path, "mods-disabled/watermedia.jar");
    assert!(!file.enabled);
    assert!(file.record.unwrap().client_only);
}
