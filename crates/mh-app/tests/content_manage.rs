//! Bật, tắt, gỡ vào thùng rác và tải lên mod/plugin (P3 PR-17, phần 2).

#![allow(clippy::unwrap_used, clippy::panic)]

mod content_support;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use content_support::{FakeModCatalog, WAIT, build_jar, finish_job, make_server, test_app};
use mh_app::content::{CatalogSource, ContentCatalog, SideSupport};
use mh_app::{App, AppEvent, CreateRequest, JobState};
use mh_core::{ErrorCode, LoaderKind, McVersion, ServerId, ServerName, ServerState};
use mh_store::{ContentManifest, ContentSource, PendingContent, ProjectId, ServerStore};

fn server_paths(app: &App, id: ServerId) -> mh_store::ServerPaths {
    ServerStore::new(app.dirs().servers(), app.dirs().trash()).paths(id)
}

fn find_trashed_file(trash_dir: &Path, id: ServerId, rel: &str) -> Option<PathBuf> {
    let files_dir = trash_dir.join(id.to_string()).join("files");
    for entry in std::fs::read_dir(files_dir).ok()?.flatten() {
        let candidate = entry.path().join(rel);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

async fn install_from_fake(app: &Arc<App>, id: ServerId, project: &str) {
    let job = app
        .install_content(id, CatalogSource::Modrinth, project, None)
        .unwrap();
    assert_eq!(finish_job(app, job).await.state, JobState::Succeeded);
}

fn pending_entry(file: &str, name: &str, pid: u64, fid: u64, sha1: String) -> PendingContent {
    PendingContent {
        path: format!("mods/{file}"),
        name: Some(name.to_owned()),
        project_id: pid,
        file_id: fid,
        sha1,
        page_url: format!("https://www.curseforge.com/minecraft/mc-mods/{name}/download/{fid}"),
        extra: Default::default(),
    }
}

#[tokio::test]
async fn toggle_content_enabled_and_error_cases() {
    let mut cat = FakeModCatalog::default();
    cat.add_project("M", "m", "Mod", SideSupport::Required);
    cat.add_project("C", "c", "ClientMod", SideSupport::Unsupported);
    cat.add_version(
        "vM",
        "M",
        "mod.jar",
        &["fabric"],
        vec![],
        build_jar(None, "M"),
    );
    cat.add_version(
        "vC",
        "C",
        "client.jar",
        &["fabric"],
        vec![],
        build_jar(Some("client"), "C"),
    );

    let (_tmp, app) = test_app(Arc::new(cat) as Arc<dyn ContentCatalog>);
    let vanilla = make_server(&app, LoaderKind::Vanilla);
    assert_eq!(
        app.set_content_enabled(vanilla, "mod.jar", false)
            .await
            .unwrap_err()
            .code,
        ErrorCode::ContentIncompatible
    );

    let id = make_server(&app, LoaderKind::Fabric);
    let paths = server_paths(&app, id);
    install_from_fake(&app, id, "M").await;
    install_from_fake(&app, id, "C").await;

    let mut events = app.subscribe_events();

    // Tắt mod đang bật -> sang mods-disabled/, content.toml giữ nguyên path dạng bật.
    app.set_content_enabled(id, "mod.jar", false).await.unwrap();
    assert!(!paths.server_dir.join("mods/mod.jar").exists());
    assert!(paths.server_dir.join("mods-disabled/mod.jar").is_file());
    assert_eq!(events.try_recv().unwrap(), AppEvent::ServerChanged(id));
    let manifest = ContentManifest::load(&paths.content_file).unwrap();
    assert!(manifest.file.iter().any(|r| r.path == "mods/mod.jar"));

    // Bật lại mod đang tắt và mod chỉ chạy client -> cả hai nằm ở mods/ và không tự tắt lại.
    app.set_content_enabled(id, "mod.jar", true).await.unwrap();
    app.set_content_enabled(id, "client.jar", true)
        .await
        .unwrap();
    let list = app.content(id).await.unwrap();
    assert!(list.files.iter().all(|f| f.enabled && !f.manual));

    // Tắt file không có -> NOT_FOUND.
    assert_eq!(
        app.set_content_enabled(id, "missing.jar", false)
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );

    // Trùng tên ở đích -> FILE_EXISTS, không ghi đè file đích.
    std::fs::write(
        paths.server_dir.join("mods-disabled/mod.jar"),
        build_jar(None, "dup"),
    )
    .unwrap();
    assert_eq!(
        app.set_content_enabled(id, "mod.jar", false)
            .await
            .unwrap_err()
            .code,
        ErrorCode::FileExists
    );
    assert!(paths.server_dir.join("mods/mod.jar").is_file());
    assert!(paths.server_dir.join("mods-disabled/mod.jar").is_file());

    // Hoạt động đúng với Paper (`plugins/` và `plugins-disabled/`).
    let paper = make_server(&app, LoaderKind::Paper);
    let paper_paths = server_paths(&app, paper);
    std::fs::create_dir_all(paper_paths.server_dir.join("plugins")).unwrap();
    std::fs::write(
        paper_paths.server_dir.join("plugins/p.jar"),
        build_jar(None, "P"),
    )
    .unwrap();
    app.set_content_enabled(paper, "p.jar", false)
        .await
        .unwrap();
    assert!(
        paper_paths
            .server_dir
            .join("plugins-disabled/p.jar")
            .is_file()
    );
}

#[tokio::test]
async fn remove_content_from_enabled_and_disabled_moves_to_trash() {
    let mut cat = FakeModCatalog::default();
    cat.add_project("A", "a", "Mod A", SideSupport::Required);
    cat.add_project("B", "b", "Mod B", SideSupport::Unsupported);
    cat.add_version(
        "vA",
        "A",
        "a.jar",
        &["fabric"],
        vec![],
        build_jar(None, "A"),
    );
    cat.add_version(
        "vB",
        "B",
        "b.jar",
        &["fabric"],
        vec![],
        build_jar(None, "B"),
    );

    let (_tmp, app) = test_app(Arc::new(cat));
    let vanilla = make_server(&app, LoaderKind::Vanilla);
    assert_eq!(
        app.remove_content(vanilla, "a.jar").await.unwrap_err().code,
        ErrorCode::ContentIncompatible
    );

    let id = make_server(&app, LoaderKind::Fabric);
    let paths = server_paths(&app, id);
    install_from_fake(&app, id, "A").await;
    install_from_fake(&app, id, "B").await;
    let mut events = app.subscribe_events();

    // Gỡ từ thư mục bật.
    app.remove_content(id, "a.jar").await.unwrap();
    assert!(!paths.server_dir.join("mods/a.jar").exists());
    assert!(find_trashed_file(&app.dirs().trash(), id, "mods/a.jar").is_some());
    assert_eq!(events.try_recv().unwrap(), AppEvent::ServerChanged(id));

    // Gỡ từ thư mục tắt.
    app.remove_content(id, "b.jar").await.unwrap();
    assert!(!paths.server_dir.join("mods-disabled/b.jar").exists());
    assert!(find_trashed_file(&app.dirs().trash(), id, "mods-disabled/b.jar").is_some());
    assert_eq!(events.try_recv().unwrap(), AppEvent::ServerChanged(id));

    let manifest = ContentManifest::load(&paths.content_file).unwrap();
    assert!(manifest.file.is_empty());

    // Gỡ file không có -> NOT_FOUND.
    assert_eq!(
        app.remove_content(id, "a.jar").await.unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[tokio::test]
async fn upload_content_creates_record_clears_pending_and_rejects_corrupt_jar() {
    let (tmp, app) = test_app(Arc::new(FakeModCatalog::default()));
    let vanilla = make_server(&app, LoaderKind::Vanilla);
    let vanilla_src = tmp.path().join("v.jar");
    std::fs::write(&vanilla_src, build_jar(None, "V")).unwrap();
    assert_eq!(
        app.upload_content(vanilla, "v.jar", &vanilla_src)
            .await
            .unwrap_err()
            .code,
        ErrorCode::ContentIncompatible
    );

    let id = make_server(&app, LoaderKind::Fabric);
    let paths = server_paths(&app, id);
    let mut events = app.subscribe_events();

    // Tải lên file jar hợp lệ -> tạo bản ghi `upload` kèm sha1.
    let src1 = tmp.path().join("up1.jar");
    std::fs::write(&src1, build_jar(None, "U1")).unwrap();
    let uploaded = app.upload_content(id, "custom.jar", &src1).await.unwrap();
    assert_eq!(uploaded.file_name, "custom.jar");
    assert_eq!(uploaded.path, "mods/custom.jar");
    assert!(uploaded.enabled);
    assert!(!uploaded.manual);
    assert_eq!(
        uploaded.record.as_ref().map(|r| r.source),
        Some(ContentSource::Upload)
    );
    assert!(paths.server_dir.join("mods/custom.jar").is_file());
    assert!(!src1.exists());
    assert_eq!(events.try_recv().unwrap(), AppEvent::ServerChanged(id));

    // Trùng tên trong thư mục bật -> FILE_EXISTS, không ghi đè.
    let src_dup = tmp.path().join("dup.jar");
    std::fs::write(&src_dup, build_jar(None, "DUP")).unwrap();
    assert_eq!(
        app.upload_content(id, "custom.jar", &src_dup)
            .await
            .unwrap_err()
            .code,
        ErrorCode::FileExists
    );
    assert!(src_dup.is_file());

    // Jar hỏng -> INVALID_INPUT và không để lại file nào trong mods/.
    let bad_src = tmp.path().join("bad.jar");
    std::fs::write(&bad_src, b"not a zip archive").unwrap();
    assert_eq!(
        app.upload_content(id, "broken.jar", &bad_src)
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    assert!(!paths.server_dir.join("mods/broken.jar").exists());

    // Tải lên khớp sha1 một mục `[[pending]]`: xóa mục pending và đặt đúng tên trong `pending.path`.
    let pending_bytes = build_jar(None, "PENDING");
    let pending_src = tmp.path().join("downloaded.jar");
    std::fs::write(&pending_src, &pending_bytes).unwrap();
    let pending_sha1 = mh_store::inspect_jar_file(&pending_src).unwrap().sha1;

    let mut manifest = ContentManifest::load(&paths.content_file).unwrap();
    manifest.pending.push(pending_entry(
        "optifine.jar",
        "OptiFine",
        238222,
        4912345,
        pending_sha1.clone(),
    ));
    manifest.save(&paths.content_file).unwrap();

    let resolved = app
        .upload_content(id, "OptiFine (1).jar", &pending_src)
        .await
        .unwrap();
    assert_eq!(resolved.file_name, "optifine.jar");
    assert_eq!(resolved.path, "mods/optifine.jar");
    assert!(paths.server_dir.join("mods/optifine.jar").is_file());
    assert!(!paths.server_dir.join("mods/OptiFine (1).jar").exists());

    let list = app.content(id).await.unwrap();
    assert!(list.pending.is_empty());
    assert!(
        list.files
            .iter()
            .any(|f| f.file_name == "optifine.jar" && f.sha1 == pending_sha1)
    );
}

#[tokio::test]
async fn invalid_file_names_are_rejected_for_all_operations() {
    let (tmp, app) = test_app(Arc::new(FakeModCatalog::default()));
    let id = make_server(&app, LoaderKind::Fabric);
    let src = tmp.path().join("ok.jar");
    std::fs::write(&src, build_jar(None, "OK")).unwrap();

    for bad in [
        "../evil.jar",
        "a/b.jar",
        "a\\b.jar",
        "..",
        ".",
        "",
        "not-jar.txt",
    ] {
        assert_eq!(
            app.set_content_enabled(id, bad, false)
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput,
            "toggle với tên `{bad}`"
        );
        assert_eq!(
            app.remove_content(id, bad).await.unwrap_err().code,
            ErrorCode::InvalidInput,
            "remove với tên `{bad}`"
        );
        assert_eq!(
            app.upload_content(id, bad, &src).await.unwrap_err().code,
            ErrorCode::InvalidInput,
            "upload với tên `{bad}`"
        );
    }
}

#[tokio::test]
async fn running_server_blocks_toggle_remove_and_upload_with_server_busy() {
    let (tmp, app) = test_app(Arc::new(FakeModCatalog::default()));
    let created = app
        .create(CreateRequest {
            name: ServerName::parse("BusyManage").unwrap(),
            loader: LoaderKind::Paper,
            minecraft: McVersion::parse("1.21.4").unwrap(),
            loader_version: None,
            memory_mb: None,
            port: None,
        })
        .unwrap();
    assert_eq!(
        finish_job(&app, created.job_id).await.state,
        JobState::Succeeded
    );
    app.accept_eula(created.server_id).unwrap();
    app.start(created.server_id).await.unwrap();
    let deadline = tokio::time::Instant::now() + WAIT;
    while app.get(created.server_id).unwrap().state != ServerState::Running {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let src = tmp.path().join("plug.jar");
    std::fs::write(&src, build_jar(None, "P")).unwrap();

    assert_eq!(
        app.set_content_enabled(created.server_id, "plug.jar", false)
            .await
            .unwrap_err()
            .code,
        ErrorCode::ServerBusy
    );
    assert_eq!(
        app.remove_content(created.server_id, "plug.jar")
            .await
            .unwrap_err()
            .code,
        ErrorCode::ServerBusy
    );
    assert_eq!(
        app.upload_content(created.server_id, "plug.jar", &src)
            .await
            .unwrap_err()
            .code,
        ErrorCode::ServerBusy
    );

    app.stop(created.server_id).await.unwrap();
    let deadline = tokio::time::Instant::now() + WAIT;
    while app.get(created.server_id).unwrap().state.has_process() {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn upload_pending_strict_mode_checks_hash_records_curseforge_and_batches() {
    let (tmp, app) = test_app(Arc::new(FakeModCatalog::default()));
    let id = make_server(&app, LoaderKind::Fabric);
    app.accept_eula(id).unwrap();
    let paths = server_paths(&app, id);

    let first_bytes = build_jar(None, "FIRST_PENDING");
    let first_src = tmp.path().join("first-download (1).jar");
    std::fs::write(&first_src, &first_bytes).unwrap();
    let first_sha1 = mh_store::inspect_jar_file(&first_src).unwrap().sha1;

    let second_bytes = build_jar(None, "SECOND_PENDING");
    let second_probe = tmp.path().join("second-probe.jar");
    std::fs::write(&second_probe, &second_bytes).unwrap();
    let second_sha1 = mh_store::inspect_jar_file(&second_probe).unwrap().sha1;

    let mut manifest = ContentManifest::load(&paths.content_file).unwrap();
    manifest.pending.extend([
        pending_entry("optifine.jar", "OptiFine", 238222, 4912345, first_sha1),
        pending_entry("restricted.jar", "Restricted", 777001, 7770001, second_sha1),
    ]);
    manifest.save(&paths.content_file).unwrap();

    // Khi còn mục chờ thì start bị CURSEFORGE_FILES_PENDING chặn.
    assert_eq!(
        app.start(id).await.unwrap_err().code,
        ErrorCode::CurseforgeFilesPending
    );

    // File sai hash bị từ chối kèm tên file đúng đang chờ, không ghi vào mods/.
    let wrong_src = tmp.path().join("wrong.jar");
    std::fs::write(&wrong_src, build_jar(None, "WRONG")).unwrap();
    let wrong_err = app
        .upload_pending_content(id, "wrong.jar", &wrong_src)
        .await
        .unwrap_err();
    assert_eq!(wrong_err.code, ErrorCode::UploadHashMismatch);
    assert_eq!(
        wrong_err.params["expected"],
        serde_json::json!("optifine.jar, restricted.jar")
    );
    assert!(!paths.server_dir.join("mods/wrong.jar").exists());
    assert!(!paths.server_dir.join("mods/optifine.jar").exists());

    // Batch nhiều file: một file đúng khớp thứ hai, một file sai -> kết quả riêng từng file.
    let second_src = tmp.path().join("UserRenamed.jar");
    std::fs::write(&second_src, &second_bytes).unwrap();
    let wrong_batch_src = tmp.path().join("also-wrong.jar");
    std::fs::write(&wrong_batch_src, build_jar(None, "WRONG_2")).unwrap();
    let batch = app
        .upload_pending_batch(
            id,
            vec![
                ("also-wrong.jar".to_owned(), wrong_batch_src),
                ("UserRenamed.jar".to_owned(), second_src),
            ],
        )
        .await
        .unwrap();
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[0].file_name, "also-wrong.jar");
    let batch_err = batch[0].outcome.as_ref().unwrap_err();
    assert_eq!(batch_err.code, ErrorCode::UploadHashMismatch);
    assert_eq!(
        batch_err.params["expected"],
        serde_json::json!("optifine.jar, restricted.jar")
    );
    let ok = batch[1].outcome.as_ref().unwrap();
    assert_eq!(ok.file_name, "restricted.jar");
    assert_eq!(ok.path, "mods/restricted.jar");
    let rec = ok.record.as_ref().unwrap();
    assert_eq!(rec.source, ContentSource::Curseforge);
    assert_eq!(rec.project_id, Some(ProjectId::Int(777001)));
    assert_eq!(rec.version_id.as_deref(), Some("7770001"));

    // Nạp nốt mục chờ cuối cùng -> hết chờ, start không còn bị chặn.
    let resolved = app
        .upload_pending_content(id, "first-download (1).jar", &first_src)
        .await
        .unwrap();
    assert_eq!(resolved.file_name, "optifine.jar");
    assert_eq!(
        resolved.record.as_ref().map(|r| r.source),
        Some(ContentSource::Curseforge)
    );
    assert!(app.content(id).await.unwrap().pending.is_empty());
    assert!(!app.preflight(id).await.unwrap().is_blocking());

    app.start(id).await.unwrap();
    let deadline = tokio::time::Instant::now() + WAIT;
    while app.get(id).unwrap().state != ServerState::Running {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    app.stop(id).await.unwrap();
    while app.get(id).unwrap().state.has_process() {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
