//! Mod cài theo modpack (`source = "modpack"`) vẫn được tính là đã cài khi thêm mod lẻ.

#![allow(clippy::unwrap_used, clippy::panic)]

mod content_support;

use std::sync::Arc;

use content_support::{FakeModCatalog, build_jar, dep, finish_job, make_server, test_app};
use mh_app::JobState;
use mh_app::content::{CatalogSource, DependencyType, SideSupport};
use mh_core::LoaderKind;
use mh_store::{ContentManifest, ContentSource, ServerStore};

#[tokio::test]
async fn modpack_records_count_as_installed_and_are_replaced() {
    let mut cat = FakeModCatalog::default();
    cat.add_project("A", "a", "A", SideSupport::Required);
    cat.add_project("B", "b", "B", SideSupport::Required);
    cat.add_version(
        "vA",
        "A",
        "a.jar",
        &["fabric"],
        vec![dep("B", None, DependencyType::Required)],
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
    let id = make_server(&app, LoaderKind::Fabric);
    let src = CatalogSource::Modrinth;

    // Giả lập B đến từ modpack: cài rồi đổi bản ghi như `finalize_modpack` ghi.
    let job = app.install_content(id, src, "B", None).unwrap();
    assert_eq!(finish_job(&app, job).await.state, JobState::Succeeded);
    let paths = ServerStore::new(app.dirs().servers(), app.dirs().trash()).paths(id);
    let mut manifest = ContentManifest::load(&paths.content_file).unwrap();
    for rec in &mut manifest.file {
        rec.source = ContentSource::Modpack;
        rec.from_modpack = true;
    }
    manifest.save(&paths.content_file).unwrap();

    let plan_a = app.plan_content_install(id, src, "A", None).await.unwrap();
    let ids: Vec<_> = plan_a.items.iter().map(|i| i.project_id.as_str()).collect();
    assert_eq!(ids, ["A"], "dependency B của pack không được tải thêm");

    let plan_b = app.plan_content_install(id, src, "B", None).await.unwrap();
    assert!(plan_b.items[0].replaces_existing);
    let job = app.install_content(id, src, "B", None).unwrap();
    assert_eq!(finish_job(&app, job).await.state, JobState::Succeeded);
    let manifest = ContentManifest::load(&paths.content_file).unwrap();
    let b: Vec<_> = manifest
        .file
        .iter()
        .filter(|r| r.project_id.as_ref().is_some_and(|p| p.to_string() == "B"))
        .collect();
    assert_eq!(b.len(), 1, "bản của pack được thay, không có hai bản B");
    assert_eq!(b[0].source, ContentSource::Modrinth);
    let jars = std::fs::read_dir(paths.server_dir.join("mods"))
        .unwrap()
        .count();
    assert_eq!(jars, 1);
}
