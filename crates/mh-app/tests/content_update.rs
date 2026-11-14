//! Cập nhật mod giữ lựa chọn bật/tắt của người dùng (spec P3, mục 3.5).

#![allow(clippy::unwrap_used, clippy::panic)]

mod content_support;

use std::sync::Arc;

use content_support::{FakeModCatalog, build_jar, finish_job, make_server, test_app};
use mh_app::content::{CatalogSource, SideSupport};
use mh_app::{App, JobState};
use mh_core::{LoaderKind, ServerId};
use mh_store::ServerStore;

async fn install(app: &Arc<App>, id: ServerId, version: &str) {
    let job = app
        .install_content(id, CatalogSource::Modrinth, "M", Some(version.to_owned()))
        .unwrap();
    assert_eq!(finish_job(app, job).await.state, JobState::Succeeded);
}

#[tokio::test]
async fn update_keeps_user_enable_choice() {
    let mut cat = FakeModCatalog::default();
    // Mod chỉ chạy client theo Modrinth: lần cài đầu vào thư mục tắt.
    cat.add_project("M", "m", "Mod", SideSupport::Unsupported);
    for (vid, file) in [("v1", "m-1.jar"), ("v2", "m-2.jar"), ("v3", "m-3.jar")] {
        cat.add_version(vid, "M", file, &["fabric"], vec![], build_jar(None, vid));
    }
    let (_tmp, app) = test_app(Arc::new(cat));
    let id = make_server(&app, LoaderKind::Fabric);
    let server = ServerStore::new(app.dirs().servers(), app.dirs().trash())
        .paths(id)
        .server_dir;

    install(&app, id, "v1").await;
    assert!(server.join("mods-disabled/m-1.jar").is_file());

    // Người dùng tự bật: bản cập nhật phải ở lại `mods/`, không bị tắt lại.
    std::fs::create_dir_all(server.join("mods")).unwrap();
    std::fs::rename(
        server.join("mods-disabled/m-1.jar"),
        server.join("mods/m-1.jar"),
    )
    .unwrap();
    install(&app, id, "v2").await;
    assert!(server.join("mods/m-2.jar").is_file());
    assert!(!server.join("mods-disabled/m-2.jar").exists());
    assert!(!server.join("mods/m-1.jar").exists());

    // Người dùng tắt lại: bản cập nhật sau giữ ở thư mục tắt.
    std::fs::rename(
        server.join("mods/m-2.jar"),
        server.join("mods-disabled/m-2.jar"),
    )
    .unwrap();
    install(&app, id, "v3").await;
    assert!(server.join("mods-disabled/m-3.jar").is_file());
    assert!(!server.join("mods/m-3.jar").exists());
}
