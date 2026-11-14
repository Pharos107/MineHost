//! Danh sách, lập kế hoạch và cài đặt mod/plugin (P3 PR-17).

#![allow(clippy::unwrap_used, clippy::panic)]

mod content_support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use content_support::{
    FakeJava, FakeModCatalog, FakeServerCatalog, WAIT, build_jar, dep, finish_job, make_server,
    sha512_hex, test_app,
};
use mh_app::content::{
    CatalogSource, ContentCatalog, ContentFileInfo, DependencyType, Modrinth, ModrinthCatalog,
    SideSupport,
};
use mh_app::jobs::JobKind;
use mh_app::{App, AppDeps, CreateRequest, JobState, MetricsConfig};
use mh_core::{ContentKind, ErrorCode, LoaderKind, McVersion, ServerName, ServerState};
use mh_net::{CancellationToken, HostAllowlist, HttpClient, NetConfig};
use mh_platform::DataDirs;
use mh_store::{ContentManifest, ContentSource, PendingContent, ServerStore};
use mh_supervisor::SupervisorConfig;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn content_list_returns_files_and_pending_and_rejects_vanilla() {
    let (_tmp, app) = test_app(Arc::new(FakeModCatalog::default()));
    let vanilla = make_server(&app, LoaderKind::Vanilla);
    let err = app.content(vanilla).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::ContentIncompatible);

    let fabric = make_server(&app, LoaderKind::Fabric);
    let paths = ServerStore::new(app.dirs().servers(), app.dirs().trash()).paths(fabric);
    std::fs::create_dir_all(paths.server_dir.join("mods")).unwrap();
    std::fs::write(paths.server_dir.join("mods/a.jar"), build_jar(None, "a")).unwrap();
    let mut manifest = ContentManifest::default();
    manifest.pending.push(PendingContent {
        path: "mods/cf.jar".to_owned(),
        project_id: 99,
        file_id: 1,
        sha1: "a".repeat(40),
        page_url: "https://example.invalid".to_owned(),
        extra: Default::default(),
    });
    manifest.save(&paths.content_file).unwrap();

    let list = app.content(fabric).await.unwrap();
    assert_eq!(list.kind, ContentKind::Mods);
    assert_eq!(list.files.len(), 1);
    assert_eq!(list.files[0].file_name, "a.jar");
    assert_eq!(list.pending.len(), 1);
    assert_eq!(list.pending[0].project_id, 99);
}

#[tokio::test]
async fn installs_two_level_deps_and_skips_optional_embedded() {
    let mut cat = FakeModCatalog::default();
    cat.add_project("A", "mod-a", "Mod A", SideSupport::Required);
    cat.add_project("B", "mod-b", "Mod B", SideSupport::Required);
    cat.add_project("C", "mod-c", "Mod C", SideSupport::Required);
    cat.add_project("OPT", "opt", "Opt", SideSupport::Required);
    cat.add_project("EMB", "emb", "Emb", SideSupport::Required);
    cat.add_version(
        "vA",
        "A",
        "mod-a.jar",
        &["fabric"],
        vec![
            dep("B", Some("vB-pin"), DependencyType::Required),
            dep("OPT", None, DependencyType::Optional),
            dep("EMB", None, DependencyType::Embedded),
        ],
        build_jar(None, "A"),
    );
    cat.add_version(
        "vB-pin",
        "B",
        "mod-b.jar",
        &["fabric"],
        vec![dep("C", None, DependencyType::Required)],
        build_jar(None, "B"),
    );
    cat.add_version(
        "vC",
        "C",
        "mod-c.jar",
        &["fabric"],
        vec![],
        build_jar(None, "C"),
    );
    cat.add_version(
        "vO",
        "OPT",
        "opt.jar",
        &["fabric"],
        vec![],
        build_jar(None, "O"),
    );
    let cat = Arc::new(cat);
    let (_tmp, app) = test_app(Arc::clone(&cat) as Arc<dyn ContentCatalog>);
    let id = make_server(&app, LoaderKind::Fabric);

    let plan = app
        .plan_content_install(id, CatalogSource::Modrinth, "mod-a", None)
        .await
        .unwrap();
    assert_eq!(cat.downloads.load(Ordering::SeqCst), 0);
    let ids: Vec<_> = plan.items.iter().map(|i| i.version_id.as_str()).collect();
    assert_eq!(ids, vec!["vA", "vB-pin", "vC"]);
    assert!(!plan.items[0].is_dependency);
    assert!(plan.items[1].is_dependency);

    let job_id = app
        .install_content(id, CatalogSource::Modrinth, "mod-a", None)
        .unwrap();
    // Không kiểm từng bước trung gian: kênh `watch` chỉ giữ giá trị mới nhất, máy nhanh sẽ bỏ qua
    // vài bước. Số file đã tải mới là thứ quan sát được chắc chắn.
    let snap = finish_job(&app, job_id).await;
    assert_eq!(snap.kind, JobKind::InstallContent);
    assert_eq!(snap.state, JobState::Succeeded);
    assert_eq!(cat.downloads.load(Ordering::SeqCst), 3);

    let list = app.content(id).await.unwrap();
    assert_eq!(list.files.len(), 3);
    assert!(list.files.iter().all(|f| f.enabled));
    let manifest = ContentManifest::load(
        &ServerStore::new(app.dirs().servers(), app.dirs().trash())
            .paths(id)
            .content_file,
    )
    .unwrap();
    assert_eq!(manifest.file.len(), 3);
    assert!(
        manifest
            .file
            .iter()
            .all(|r| r.source == ContentSource::Modrinth && !r.sha1.is_empty())
    );
}

#[tokio::test]
async fn circular_dependencies_terminate_and_install_both() {
    let mut cat = FakeModCatalog::default();
    cat.add_project("A", "slug-a", "A", SideSupport::Required);
    cat.add_project("B", "slug-b", "B", SideSupport::Required);
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
        vec![dep("slug-a", None, DependencyType::Required)],
        build_jar(None, "B"),
    );
    let (_tmp, app) = test_app(Arc::new(cat));
    let id = make_server(&app, LoaderKind::Fabric);

    let job = app
        .install_content(id, CatalogSource::Modrinth, "slug-a", None)
        .unwrap();
    assert_eq!(finish_job(&app, job).await.state, JobState::Succeeded);
    let list = app.content(id).await.unwrap();
    assert_eq!(list.files.len(), 2);
}

#[tokio::test]
async fn rejects_incompatible_loader_conflicts_and_skips_installed_dep() {
    let mut cat = FakeModCatalog::default();
    cat.add_project("A", "a", "A", SideSupport::Required);
    cat.add_project("B", "b", "B", SideSupport::Required);
    cat.add_project("BAD", "bad", "Bad", SideSupport::Required);
    cat.add_project("CON", "con", "Conflict", SideSupport::Required);
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
    cat.add_version(
        "vBad",
        "BAD",
        "bad.jar",
        &["forge"],
        vec![],
        build_jar(None, "BAD"),
    );
    cat.add_version(
        "vConInstalled",
        "CON",
        "con.jar",
        &["fabric"],
        vec![dep("B", None, DependencyType::Incompatible)],
        build_jar(None, "CON"),
    );
    cat.add_version(
        "vConPlan",
        "CON",
        "con2.jar",
        &["fabric"],
        vec![
            dep("A", None, DependencyType::Required),
            dep("B", None, DependencyType::Incompatible),
        ],
        build_jar(None, "CON2"),
    );

    let (_tmp, app) = test_app(Arc::new(cat));
    let id = make_server(&app, LoaderKind::Fabric);

    let src = CatalogSource::Modrinth;
    assert_eq!(
        app.plan_content_install(id, src, "BAD", None)
            .await
            .unwrap_err()
            .code,
        ErrorCode::ContentIncompatible
    );
    assert_eq!(
        app.plan_content_install(id, src, "BAD", Some("vBad"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::ContentIncompatible
    );
    assert_eq!(
        app.plan_content_install(id, src, "CON", Some("vConPlan"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::ContentIncompatible
    );

    // Cài B trước, rồi A bỏ qua B; CON incompatible với B đang cài -> CONTENT_INCOMPATIBLE.
    let job_b = app.install_content(id, src, "B", None).unwrap();
    assert_eq!(finish_job(&app, job_b).await.state, JobState::Succeeded);
    let plan_a = app.plan_content_install(id, src, "A", None).await.unwrap();
    assert_eq!(plan_a.items.len(), 1);
    assert_eq!(plan_a.items[0].project_id, "A");
    assert_eq!(
        app.plan_content_install(id, src, "CON", Some("vConInstalled"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::ContentIncompatible
    );
}

#[tokio::test]
async fn replace_client_only_and_midway_failure_behavior() {
    let mut cat = FakeModCatalog::default();
    cat.add_project("A", "a", "A", SideSupport::Required);
    cat.add_project("CLI", "cli", "ClientSide", SideSupport::Unsupported);
    cat.add_project("JAR", "jar", "JarClient", SideSupport::Optional);
    cat.add_project("FAIL", "fail", "Fail", SideSupport::Required);
    cat.add_version(
        "vA1",
        "A",
        "a-1.0.jar",
        &["fabric"],
        vec![],
        build_jar(None, "A1"),
    );
    cat.add_version(
        "vA2",
        "A",
        "a-2.0.jar",
        &["fabric"],
        vec![],
        build_jar(None, "A2"),
    );
    cat.add_version(
        "vCli",
        "CLI",
        "cli.jar",
        &["fabric"],
        vec![],
        build_jar(None, "C"),
    );
    cat.add_version(
        "vJar",
        "JAR",
        "jar.jar",
        &["fabric"],
        vec![],
        build_jar(Some("client"), "J"),
    );
    cat.add_version(
        "vFailRoot",
        "A",
        "root.jar",
        &["fabric"],
        vec![dep("FAIL", None, DependencyType::Required)],
        build_jar(None, "R"),
    );
    cat.add_version(
        "vFailDep",
        "FAIL",
        "fail.jar",
        &["fabric"],
        vec![],
        build_jar(None, "F"),
    );
    cat.fail_urls
        .insert("https://cdn.modrinth.com/vFailDep/fail.jar".to_owned());

    let (_tmp, app) = test_app(Arc::new(cat));
    let id = make_server(&app, LoaderKind::Fabric);

    let src = CatalogSource::Modrinth;
    // Lỗi tải giữa chừng: không để lại file nào trong mods/.
    let bad_job = app
        .install_content(id, src, "A", Some("vFailRoot".to_owned()))
        .unwrap();
    assert!(matches!(
        finish_job(&app, bad_job).await.state,
        JobState::Failed {
            code: ErrorCode::DownloadFailed,
            ..
        }
    ));
    let paths = ServerStore::new(app.dirs().servers(), app.dirs().trash()).paths(id);
    assert!(app.content(id).await.unwrap().files.is_empty());
    assert!(!paths.content_file.exists());

    // Cài v1 rồi thay bằng v2 -> bản cũ vào thùng rác.
    let job_a1 = app
        .install_content(id, src, "A", Some("vA1".to_owned()))
        .unwrap();
    assert_eq!(finish_job(&app, job_a1).await.state, JobState::Succeeded);
    let job_a2 = app
        .install_content(id, src, "A", Some("vA2".to_owned()))
        .unwrap();
    assert_eq!(finish_job(&app, job_a2).await.state, JobState::Succeeded);
    assert!(!paths.server_dir.join("mods/a-1.0.jar").exists());
    assert!(paths.server_dir.join("mods/a-2.0.jar").exists());
    assert!(app.dirs().trash().join(id.to_string()).exists());

    // Hai lần cài đồng thời trên cùng server được khóa tuần tự, không ghi đè mất `content.toml`.
    let j1 = app.install_content(id, src, "CLI", None).unwrap();
    let j2 = app.install_content(id, src, "JAR", None).unwrap();
    assert_eq!(finish_job(&app, j1).await.state, JobState::Succeeded);
    assert_eq!(finish_job(&app, j2).await.state, JobState::Succeeded);
    let list = app.content(id).await.unwrap();
    assert_eq!(list.files.len(), 3);
    assert!(list.files.iter().any(|f| f.file_name == "cli.jar"
        && !f.enabled
        && f.record.as_ref().is_some_and(|r| r.client_only)));
    assert!(list.files.iter().any(|f| f.file_name == "jar.jar"
        && !f.enabled
        && f.record.as_ref().is_some_and(|r| r.client_only)));
}

#[tokio::test]
async fn running_server_blocks_install_with_server_busy() {
    let mut cat = FakeModCatalog::default();
    cat.add_project("P", "plug", "Plug", SideSupport::Required);
    cat.add_version(
        "vP",
        "P",
        "plug.jar",
        &["paper"],
        vec![],
        build_jar(None, "P"),
    );
    let (_tmp, app) = test_app(Arc::new(cat));
    let created = app
        .create(CreateRequest {
            name: ServerName::parse("Busy").unwrap(),
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

    let err = app
        .install_content(created.server_id, CatalogSource::Modrinth, "P", None)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::ServerBusy);

    app.stop(created.server_id).await.unwrap();
    let deadline = tokio::time::Instant::now() + WAIT;
    while app.get(created.server_id).unwrap().state.has_process() {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn offline_default_errors_and_real_modrinth_catalog_uses_hash_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let deps = AppDeps {
        catalog: Arc::new(FakeServerCatalog),
        java: Arc::new(FakeJava),
        port_free: Arc::new(|_| true),
        free_space: Arc::new(|_| Some(100 * 1024 * 1024 * 1024)),
        total_memory_mb: 16 * 1024,
        supervisor: SupervisorConfig::default(),
        save_flush_timeout: Duration::from_secs(60),
        metrics: MetricsConfig::default(),
    };
    let offline_app = App::with_deps(dirs.clone(), deps).unwrap();
    let offline_id = make_server(&offline_app, LoaderKind::Fabric);
    assert_eq!(
        offline_app
            .plan_content_install(offline_id, CatalogSource::Modrinth, "sodium", None)
            .await
            .unwrap_err()
            .code,
        ErrorCode::NetworkUnavailable
    );

    let body = build_jar(None, "cached");
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_srv = Arc::clone(&hits);
    let body_srv = body.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let srv = tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let mut buf = [0u8; 512];
            let _ = sock.read(&mut buf).await;
            hits_srv.fetch_add(1, Ordering::SeqCst);
            let hdr = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body_srv.len()
            );
            let _ = sock.write_all(hdr.as_bytes()).await;
            let _ = sock.write_all(&body_srv).await;
        }
    });

    let http = Arc::new(HttpClient::new(NetConfig::new(dirs.cache())).unwrap());
    let hosts = HostAllowlist::new(&["127.0.0.1"]);
    let modrinth =
        Modrinth::with_base_url(Arc::clone(&http), format!("http://{addr}")).with_hosts(hosts);
    let real_cat = ModrinthCatalog::new(http).with_modrinth(modrinth, hosts);
    let file = ContentFileInfo {
        url: Some(format!("http://{addr}/mod.jar")),
        filename: "mod.jar".to_owned(),
        size: body.len() as u64,
        sha1: Some("0".repeat(40)),
        sha512: Some(sha512_hex(&body)),
        manual_download: false,
        download_page_url: None,
    };
    let cancel = CancellationToken::new();
    let mut noop = |_: u64, _: Option<u64>| {};
    real_cat
        .download(&file, &tmp.path().join("d1.jar"), &mut noop, &cancel)
        .await
        .unwrap();
    real_cat
        .download(&file, &tmp.path().join("d2.jar"), &mut noop, &cancel)
        .await
        .unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    srv.abort();
}
