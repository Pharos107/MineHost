//! Kiểm thử luồng cài modpack CurseForge trên chế độ `e2e` giả (không gọi mạng thật).
#![cfg(feature = "e2e")]
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use mh_app::content::{CatalogSource, ProjectType};
use mh_app::{App, JobState, ModpackRequest, ModpackSourceInput, ModpackSummary, e2e};
use mh_core::instance::ModpackSource;
use mh_core::{GamePort, ServerName};
use mh_platform::DataDirs;
use mh_store::{ContentManifest, ServerStore};

const WAIT: Duration = Duration::from_secs(10);

fn build_e2e_app() -> (tempfile::TempDir, Arc<App>) {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let mut app = App::with_deps(
        dirs,
        e2e::deps(PathBuf::from(env!("CARGO_BIN_EXE_app-fake-mc"))),
    )
    .unwrap()
    .with_curseforge(e2e::curseforge());
    if let Some(key) = e2e::curseforge_key() {
        app = app.with_curseforge_key(key);
    }
    (tmp, Arc::new(app))
}

#[tokio::test]
async fn e2e_curseforge_modpacks_install_restricted_and_server_pack() {
    let (tmp, app) = build_e2e_app();
    let hits = app
        .search_content(
            CatalogSource::Curseforge,
            "cf-",
            Some(ProjectType::Modpack),
            None,
            1,
        )
        .await
        .unwrap();
    let ids: Vec<&str> = hits.hits.iter().map(|h| h.project_id.as_str()).collect();
    assert!(
        ids.contains(&"910001") && ids.contains(&"910002"),
        "{ids:?}"
    );

    let c1 = app
        .create_from_modpack(ModpackRequest {
            source: ModpackSourceInput::Curseforge {
                project_id: "910001".to_owned(),
                version_id: Some("5900001".to_owned()),
            },
            name: ServerName::parse("E2E Pending").unwrap(),
            memory_mb: None,
            port: Some(GamePort::new(25565).unwrap()),
        })
        .unwrap();
    let mut rx1 = app.jobs().subscribe(c1.job_id).unwrap();
    let snap1 = tokio::time::timeout(WAIT, rx1.wait_for(|s| s.state != JobState::Running))
        .await
        .unwrap()
        .unwrap()
        .clone();
    assert_eq!(snap1.state, JobState::Succeeded);
    let summary: ModpackSummary = serde_json::from_value(snap1.result.unwrap()).unwrap();
    assert_eq!(
        summary.pending_curseforge,
        vec!["mods/cf-restricted-1.0.0.jar"]
    );

    let store = ServerStore::new(tmp.path().join("servers"), tmp.path().join("trash"));
    let p1 = store.paths(c1.server_id);
    assert!(
        p1.server_dir
            .join("mods/cf-fabric-api-0.110.0.jar")
            .is_file()
    );
    assert!(p1.server_dir.join("config/cf-restricted.cfg").is_file());
    assert_eq!(
        ContentManifest::load(&p1.content_file).unwrap().pending[0].path,
        "mods/cf-restricted-1.0.0.jar"
    );

    let c2 = app
        .create_from_modpack(ModpackRequest {
            source: ModpackSourceInput::Curseforge {
                project_id: "910002".to_owned(),
                version_id: Some("5900002".to_owned()),
            },
            name: ServerName::parse("E2E Server Pack").unwrap(),
            memory_mb: None,
            port: Some(GamePort::new(25566).unwrap()),
        })
        .unwrap();
    let mut rx2 = app.jobs().subscribe(c2.job_id).unwrap();
    let snap2 = tokio::time::timeout(WAIT, rx2.wait_for(|s| s.state != JobState::Running))
        .await
        .unwrap()
        .unwrap()
        .clone();
    assert_eq!(snap2.state, JobState::Succeeded);

    let s2 = store.paths(c2.server_id).server_dir;
    assert!(s2.join("mods/cf-fabric-api-0.110.0.jar").is_file());
    assert!(s2.join("config/server-pack.toml").is_file());
    assert!(!s2.join("start.sh").exists());
    assert!(!s2.join("run.bat").exists());
    assert!(!s2.join("libraries").exists());
    assert_eq!(
        app.get(c2.server_id)
            .unwrap()
            .spec
            .unwrap()
            .modpack
            .unwrap()
            .source,
        ModpackSource::Curseforge
    );
}
