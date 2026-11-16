//! Tạo server kèm mod chọn ở bước Mod và duyệt catalog khi chưa có server (spec trình duyệt mod, mục 5).

#![allow(clippy::unwrap_used, clippy::panic)]

mod content_support;

use std::sync::Arc;

use content_support::{FakeModCatalog, build_jar, finish_job, test_app};
use mh_app::content::{CatalogSource, ContentRequest, ContentTarget, SideSupport};
use mh_app::{CreateRequest, JobState};
use mh_core::{ErrorCode, LoaderKind, McVersion, ServerName};
use mh_store::ServerStore;
use serde_json::json;

fn request(loader: LoaderKind) -> CreateRequest {
    CreateRequest {
        name: ServerName::parse("Kèm mod").unwrap(),
        loader,
        minecraft: McVersion::parse("1.21.4").unwrap(),
        loader_version: None,
        memory_mb: None,
        port: None,
    }
}

fn item(project_id: &str) -> ContentRequest {
    ContentRequest {
        source: CatalogSource::Modrinth,
        project_id: project_id.to_owned(),
        version_id: None,
    }
}

fn catalog() -> FakeModCatalog {
    let mut cat = FakeModCatalog::default();
    cat.add_project("P", "plug", "Plug", SideSupport::Required);
    cat.add_project("F", "fab", "Fab", SideSupport::Required);
    cat.add_version(
        "vP",
        "P",
        "plug.jar",
        &["paper"],
        vec![],
        build_jar(None, "P"),
    );
    cat.add_version(
        "vF",
        "F",
        "fab.jar",
        &["fabric"],
        vec![],
        build_jar(None, "F"),
    );
    cat
}

#[tokio::test]
async fn create_installs_chosen_content_and_reports_failures_without_breaking_server() {
    let (_tmp, app) = test_app(Arc::new(catalog()));
    let created = app
        .create_with_content(request(LoaderKind::Paper), vec![item("P"), item("F")])
        .unwrap();
    let job = finish_job(&app, created.job_id).await;
    assert_eq!(job.state, JobState::Succeeded);
    assert_eq!(
        job.result,
        Some(json!({ "content_failed": [
            { "source": "modrinth", "project_id": "F", "code": "CONTENT_INCOMPATIBLE" }
        ] }))
    );

    let id = created.server_id;
    assert!(app.get(id).unwrap().spec.is_some());
    let plugins = ServerStore::new(app.dirs().servers(), app.dirs().trash())
        .paths(id)
        .server_dir
        .join("plugins");
    assert!(plugins.join("plug.jar").is_file());
    assert!(!plugins.join("fab.jar").exists());
    let names: Vec<_> = app
        .content(id)
        .await
        .unwrap()
        .files
        .into_iter()
        .map(|f| f.file_name)
        .collect();
    assert_eq!(names, ["plug.jar"]);
}

#[tokio::test]
async fn create_checks_content_before_starting_job() {
    let (_tmp, app) = test_app(Arc::new(catalog()));
    let vanilla = app.create_with_content(request(LoaderKind::Vanilla), vec![item("P")]);
    assert_eq!(vanilla.unwrap_err().code, ErrorCode::ContentIncompatible);
    let cf = ContentRequest {
        source: CatalogSource::Curseforge,
        ..item("1")
    };
    let no_key = app.create_with_content(request(LoaderKind::Paper), vec![cf]);
    assert_eq!(no_key.unwrap_err().code, ErrorCode::CurseforgeUnavailable);
    let too_many = (0..51).map(|_| item("P")).collect();
    let err = app.create_with_content(request(LoaderKind::Paper), too_many);
    assert_eq!(err.unwrap_err().code, ErrorCode::InvalidInput);
    assert!(app.list().unwrap().is_empty());
}

#[tokio::test]
async fn target_versions_filter_by_loader_without_server() {
    let (_tmp, app) = test_app(Arc::new(catalog()));
    let target = |loader| ContentTarget {
        loader,
        minecraft: McVersion::parse("1.21.4").unwrap(),
    };
    let src = CatalogSource::Modrinth;
    let paper = app
        .target_versions(src, "P", &target(LoaderKind::Paper))
        .await;
    assert_eq!(paper.unwrap()[0].id, "vP");
    let vanilla = app
        .target_versions(src, "P", &target(LoaderKind::Vanilla))
        .await;
    assert_eq!(vanilla.unwrap_err().code, ErrorCode::ContentIncompatible);
}
