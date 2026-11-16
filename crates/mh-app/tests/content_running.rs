//! Thêm mod/plugin khi server đang chạy (spec trình duyệt mod, mục 2).

#![allow(clippy::unwrap_used, clippy::panic)]

mod content_support;

use std::sync::Arc;
use std::time::Duration;

use content_support::{FakeModCatalog, WAIT, build_jar, finish_job, test_app};
use mh_app::content::{CatalogSource, SideSupport};
use mh_app::{App, CreateRequest, JobState};
use mh_core::{ErrorCode, LoaderKind, McVersion, ServerId, ServerName, ServerState};
use mh_store::ServerStore;

fn catalog() -> FakeModCatalog {
    let mut cat = FakeModCatalog::default();
    for (pid, slug) in [("P", "plug"), ("Q", "que")] {
        cat.add_project(pid, slug, slug, SideSupport::Required);
    }
    let paper = &["paper"];
    cat.add_version("vP", "P", "plug.jar", paper, vec![], build_jar(None, "P"));
    cat.add_version("vQ1", "Q", "q1.jar", paper, vec![], build_jar(None, "Q1"));
    cat.add_version("vQ2", "Q", "q2.jar", paper, vec![], build_jar(None, "Q2"));
    cat
}

async fn wait_state(app: &App, id: ServerId, running: bool) {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let state = app.get(id).unwrap().state;
        let done = if running {
            state == ServerState::Running
        } else {
            !state.has_process()
        };
        if done {
            return;
        }
        assert!(tokio::time::Instant::now() < deadline, "chờ {state:?}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn create_paper(app: &Arc<App>) -> ServerId {
    let created = app
        .create(CreateRequest {
            name: ServerName::parse("Running").unwrap(),
            loader: LoaderKind::Paper,
            minecraft: McVersion::parse("1.21.4").unwrap(),
            loader_version: None,
            memory_mb: None,
            port: None,
        })
        .unwrap();
    assert_eq!(
        finish_job(app, created.job_id).await.state,
        JobState::Succeeded
    );
    app.accept_eula(created.server_id).unwrap();
    created.server_id
}

async fn install(app: &Arc<App>, id: ServerId, pid: &str, vid: &str) -> JobState {
    let job = app
        .install_content(id, CatalogSource::Modrinth, pid, Some(vid.to_owned()))
        .unwrap();
    finish_job(app, job).await.state
}

#[tokio::test]
async fn running_server_adds_new_content_but_never_replaces_loaded_jars() {
    let (tmp, app) = test_app(Arc::new(catalog()));
    let id = create_paper(&app).await;
    assert_eq!(install(&app, id, "Q", "vQ1").await, JobState::Succeeded);
    app.start(id).await.unwrap();
    wait_state(&app, id, true).await;
    let plugins = ServerStore::new(app.dirs().servers(), app.dirs().trash())
        .paths(id)
        .server_dir
        .join("plugins");

    // Mod mới: cài được, nằm trong `plugins/`, được đánh dấu chờ khởi động lại.
    assert_eq!(install(&app, id, "P", "vP").await, JobState::Succeeded);
    assert!(plugins.join("plug.jar").is_file());
    let list = app.content(id).await.unwrap();
    assert!(list.needs_restart.contains("plug.jar"));
    assert!(!list.needs_restart.contains("q1.jar"));

    // Bản mới của mod đã có phải thay jar đang mở: Job lỗi, bản cũ còn nguyên.
    let replaced = install(&app, id, "Q", "vQ2").await;
    assert!(
        matches!(
            replaced,
            JobState::Failed {
                code: ErrorCode::ServerBusy,
                ..
            }
        ),
        "{replaced:?}"
    );
    assert!(plugins.join("q1.jar").is_file());
    assert!(!plugins.join("q2.jar").exists());

    // Bật/tắt và gỡ vẫn bị khóa.
    let toggle = app.set_content_enabled(id, "q1.jar", false).await;
    assert_eq!(toggle.unwrap_err().code, ErrorCode::ServerBusy);
    let remove = app.remove_content(id, "q1.jar").await;
    assert_eq!(remove.unwrap_err().code, ErrorCode::ServerBusy);

    // Tải lên jar mới được; trùng tên thì báo đã có, không ghi đè.
    let src = tmp.path().join("up.jar");
    std::fs::write(&src, build_jar(None, "UP")).unwrap();
    app.upload_content(id, "up.jar", &src).await.unwrap();
    let dup = tmp.path().join("dup.jar");
    std::fs::write(&dup, build_jar(None, "DUP")).unwrap();
    let err = app.upload_content(id, "plug.jar", &dup).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::FileExists);
    let list = app.content(id).await.unwrap();
    assert_eq!(
        list.needs_restart.iter().cloned().collect::<Vec<_>>(),
        ["plug.jar", "up.jar"]
    );

    // Tắt rồi bật lại: process mới đã nạp hết, không còn mục nào chờ.
    app.stop(id).await.unwrap();
    wait_state(&app, id, false).await;
    assert!(app.content(id).await.unwrap().needs_restart.is_empty());
    app.start(id).await.unwrap();
    wait_state(&app, id, true).await;
    assert!(app.content(id).await.unwrap().needs_restart.is_empty());
    app.stop(id).await.unwrap();
    wait_state(&app, id, false).await;
}
