//! Mỗi server một thao tác chạm world tại một lúc: đang sao lưu thì sao lưu lần hai, bật hay khởi
//! động lại đều bị từ chối, và Job xong (kể cả bị hủy) là nhả khóa.

#![allow(clippy::unwrap_used, clippy::panic)]

mod content_support;

use std::sync::Arc;
use std::time::Duration;

use content_support::{FakeModCatalog, finish_job, test_app};
use mh_app::{App, BackupKind, BackupRequest, CreateRequest, JobState};
use mh_core::{ConsoleStream, ErrorCode, LoaderKind, McVersion, ServerId, ServerName, ServerState};
use mh_store::ServerStore;

const WAIT: Duration = Duration::from_secs(10);

fn manual() -> BackupRequest {
    BackupRequest {
        note: None,
        protected: false,
        kind: BackupKind::Manual,
    }
}

/// Server chạy `fake-mc` treo ở `save-all`, để Job sao lưu đứng yên ở bước chờ lưu.
async fn running_server_hanging_on_save(app: &Arc<App>) -> ServerId {
    boot(app, "--hang-on-save-all", ServerState::Running).await
}

/// Tạo server, bật với cờ `fake-mc` `flag`, chờ tới trạng thái `state`.
async fn boot(app: &Arc<App>, flag: &str, state: ServerState) -> ServerId {
    let created = app
        .create(CreateRequest {
            name: ServerName::parse("Lock").unwrap(),
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
    let id = created.server_id;
    app.accept_eula(id).unwrap();
    let store = ServerStore::new(app.dirs().servers(), app.dirs().trash());
    let mut spec = store.load(id).unwrap();
    spec.runtime.jvm_args = vec![flag.to_owned()];
    store.save(&spec).unwrap();
    app.start(id).await.unwrap();
    let status = app
        .handle(id)
        .unwrap()
        .wait_for(WAIT, |s| s.state == state)
        .await;
    assert_eq!(status.state, state, "{status:?}");
    id
}

async fn wait_for_save_off(app: &App, id: ServerId) {
    let handle = app.handle(id).unwrap();
    tokio::time::timeout(WAIT, async {
        loop {
            let sent = handle
                .console()
                .after(None)
                .into_iter()
                .any(|line| line.stream == ConsoleStream::Stdin && line.text == "save-off");
            if sent {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn backup_blocks_second_backup_and_restart_until_it_ends() {
    let (_tmp, app) = test_app(Arc::new(FakeModCatalog::default()));
    let id = running_server_hanging_on_save(&app).await;

    let first = app.backup(id, manual()).unwrap();
    wait_for_save_off(&app, id).await;

    let err = app.backup(id, manual()).unwrap_err();
    assert_eq!(err.code, ErrorCode::ServerBusy);
    assert_eq!(err.params["reason"], "world_job");
    assert_eq!(err.params["job"], "backup");
    let err = app.restart(id).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::ServerBusy);
    assert_eq!(app.handle(id).unwrap().status().state, ServerState::Running);

    // Hủy là nhả khóa: lần sao lưu sau được nhận.
    app.jobs().cancel(first.job_id).unwrap();
    assert_eq!(
        finish_job(&app, first.job_id).await.state,
        JobState::Cancelled
    );
    let again = app.backup(id, manual()).unwrap();
    app.jobs().cancel(again.job_id).unwrap();
    finish_job(&app, again.job_id).await;
    app.kill(id).await.unwrap();
}

/// Server đang bật chưa đọc lệnh: sao lưu bị từ chối ngay, không tạo Job gửi `save-off` vô ích.
#[tokio::test]
async fn backup_is_refused_while_server_is_starting() {
    let (_tmp, app) = test_app(Arc::new(FakeModCatalog::default()));
    let id = boot(&app, "--no-done", ServerState::Starting).await;
    let jobs_before = app.jobs().list(Some(id)).len();

    let err = app.backup(id, manual()).unwrap_err();
    assert_eq!(err.code, ErrorCode::ServerBusy);
    assert_eq!(err.params["reason"], "server_transition");
    assert_eq!(app.jobs().list(Some(id)).len(), jobs_before);
    app.kill(id).await.unwrap();
}
