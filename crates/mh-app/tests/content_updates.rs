//! Kiểm tra và cập nhật mod (spec P3, mục 3.9 và 5.7): nhận ra mod thêm tay qua hash, bỏ qua mod
//! của modpack, luôn có bản `before_update`, lỗi giữa chừng trả kèm mã bản backup.

#![allow(clippy::unwrap_used, clippy::panic)]

mod content_support;

use std::path::PathBuf;
use std::sync::Arc;

use content_support::{FakeModCatalog, build_jar, finish_job, make_server, test_app};
use mh_app::content::{CatalogSource, SideSupport, UpdateItem};
use mh_app::{App, BackupKind, JobState};
use mh_core::{LoaderKind, ServerId};
use mh_store::{ContentManifest, ContentRecord, ContentSource, ProjectId, ServerStore};
use serde_json::Value;

struct Setup {
    _tmp: tempfile::TempDir,
    app: Arc<App>,
    id: ServerId,
    server: PathBuf,
    content: PathBuf,
}

/// A: cài qua MineHost, có bản mới. B: chép tay bản cũ, có bản mới. C: đã mới nhất.
/// `custom.jar` không có trên nguồn nào; `pack.jar` thuộc modpack.
async fn setup(fail_b2: bool) -> Setup {
    let mut cat = FakeModCatalog::default();
    for (pid, title) in [("A", "Mod A"), ("B", "Mod B"), ("C", "Mod C")] {
        cat.add_project(pid, &pid.to_lowercase(), title, SideSupport::Required);
    }
    // Modrinth trả bản mới nhất trước; bản giả cũng thêm theo thứ tự đó.
    for (vid, pid, file) in [
        ("a2", "A", "a-2.jar"),
        ("a1", "A", "a-1.jar"),
        ("b2", "B", "b-2.jar"),
        ("b1", "B", "b-1.jar"),
        ("c1", "C", "c-1.jar"),
    ] {
        cat.add_version(vid, pid, file, &["fabric"], vec![], build_jar(None, vid));
    }
    if fail_b2 {
        cat.fail_urls
            .insert("https://cdn.modrinth.com/b2/b-2.jar".to_owned());
    }
    let b1_bytes = build_jar(None, "b1");
    let pack_bytes = build_jar(None, "a1");
    let (tmp, app) = test_app(Arc::new(cat));
    let id = make_server(&app, LoaderKind::Fabric);
    let paths = ServerStore::new(app.dirs().servers(), app.dirs().trash()).paths(id);
    for (pid, vid) in [("A", "a1"), ("C", "c1")] {
        let job = app
            .install_content(id, CatalogSource::Modrinth, pid, Some(vid.to_owned()))
            .unwrap();
        assert_eq!(finish_job(&app, job).await.state, JobState::Succeeded);
    }
    let mods = paths.server_dir.join("mods");
    std::fs::write(mods.join("b-1.jar"), b1_bytes).unwrap();
    std::fs::write(mods.join("custom.jar"), build_jar(None, "tự build")).unwrap();
    std::fs::write(mods.join("pack.jar"), &pack_bytes).unwrap();
    let mut manifest = ContentManifest::load(&paths.content_file).unwrap();
    manifest.file.push(ContentRecord {
        path: "mods/pack.jar".to_owned(),
        sha1: content_support::sha1_hex(&pack_bytes),
        source: ContentSource::Modpack,
        project_id: None,
        version_id: None,
        client_only: false,
        from_modpack: true,
        installed_at: time::OffsetDateTime::now_utc(),
        extra: Default::default(),
    });
    manifest.save(&paths.content_file).unwrap();
    Setup {
        _tmp: tmp,
        app,
        id,
        server: paths.server_dir,
        content: paths.content_file,
    }
}

async fn check(s: &Setup) -> Value {
    let job = s.app.check_updates(s.id).unwrap();
    let snap = finish_job(&s.app, job).await;
    assert_eq!(snap.state, JobState::Succeeded, "{snap:?}");
    snap.result.unwrap()
}

fn items_from(report: &Value) -> Vec<UpdateItem> {
    report["updates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| UpdateItem {
            file: u["file"].as_str().unwrap().to_owned(),
            source: CatalogSource::parse(u["source"].as_str().unwrap()).unwrap(),
            project_id: u["project_id"].as_str().unwrap().to_owned(),
            version_id: u["latest_version_id"].as_str().unwrap().to_owned(),
        })
        .collect()
}

#[tokio::test]
async fn check_finds_recorded_and_manual_mods_but_skips_modpack_files() {
    let s = setup(false).await;
    let report = check(&s).await;
    let mut found: Vec<(String, String, bool)> = report["updates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| {
            (
                u["file"].as_str().unwrap().to_owned(),
                u["latest_version_id"].as_str().unwrap().to_owned(),
                u["manual"].as_bool().unwrap(),
            )
        })
        .collect();
    found.sort();
    assert_eq!(
        found,
        vec![
            ("mods/a-1.jar".to_owned(), "a2".to_owned(), false),
            ("mods/b-1.jar".to_owned(), "b2".to_owned(), true),
        ]
    );
    assert_eq!(report["checked"], 3, "{report}");
    assert_eq!(report["unknown"], 1, "{report}");
    assert_eq!(report["from_modpack"], 1, "{report}");
    // Kiểm tra không được đổi gì trên đĩa.
    assert!(s.server.join("mods/b-1.jar").is_file());
    assert!(s.app.list_backups(s.id).unwrap().is_empty());
}

#[tokio::test]
async fn update_backs_up_first_then_replaces_old_files() {
    let s = setup(false).await;
    let items = items_from(&check(&s).await);
    let job = s.app.update_content(s.id, items).unwrap();
    let snap = finish_job(&s.app, job).await;
    assert_eq!(snap.state, JobState::Succeeded, "{snap:?}");

    for gone in ["a-1.jar", "b-1.jar"] {
        assert!(!s.server.join("mods").join(gone).exists(), "{gone}");
    }
    for kept in ["a-2.jar", "b-2.jar", "c-1.jar", "custom.jar", "pack.jar"] {
        assert!(s.server.join("mods").join(kept).is_file(), "{kept}");
    }
    let manifest = ContentManifest::load(&s.content).unwrap();
    let b = manifest
        .file
        .iter()
        .find(|r| r.path == "mods/b-2.jar")
        .unwrap();
    assert_eq!(b.project_id, Some(ProjectId::Str("B".to_owned())));
    assert_eq!(b.version_id.as_deref(), Some("b2"));
    let backups = s.app.list_backups(s.id).unwrap();
    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].kind, BackupKind::BeforeUpdate);
    assert!(backups[0].protected);
    // Hết bản mới sau khi cập nhật.
    let report = check(&s).await;
    assert_eq!(report["updates"], Value::Array(Vec::new()), "{report}");
}

#[tokio::test]
async fn failure_midway_reports_the_backup_to_go_back_to() {
    let s = setup(true).await;
    let items = items_from(&check(&s).await);
    let job = s.app.update_content(s.id, items).unwrap();
    let snap = finish_job(&s.app, job).await;
    let JobState::Failed { params, .. } = snap.state else {
        panic!("Job phải lỗi: {snap:?}");
    };
    let backups = s.app.list_backups(s.id).unwrap();
    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].kind, BackupKind::BeforeUpdate);
    assert_eq!(params["backup_id"], Value::String(backups[0].id.clone()));
    // Bản cũ của B vẫn còn: bước cài lỗi trước khi thay file.
    assert!(s.server.join("mods/b-1.jar").is_file());
}

#[tokio::test]
async fn empty_or_unknown_items_are_rejected() {
    let s = setup(false).await;
    let err = s.app.update_content(s.id, Vec::new()).unwrap_err();
    assert_eq!(err.code, mh_core::ErrorCode::InvalidInput);

    let job = s
        .app
        .update_content(
            s.id,
            vec![UpdateItem {
                file: "mods/không-có.jar".to_owned(),
                source: CatalogSource::Modrinth,
                project_id: "A".to_owned(),
                version_id: "a2".to_owned(),
            }],
        )
        .unwrap();
    let snap = finish_job(&s.app, job).await;
    assert!(
        matches!(
            snap.state,
            JobState::Failed {
                code: mh_core::ErrorCode::NotFound,
                ..
            }
        ),
        "{snap:?}"
    );
}
