//! Kiểm thử Job cài modpack (`App::create_from_modpack`, spec P3 mục 3.4, 4.1, 4.2, 5.7).

#![allow(clippy::unwrap_used, clippy::panic)]

mod support;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use mh_app::{
    App, AppDeps, JobState, MetricsConfig, ModpackRequest, ModpackSourceInput, ModpackSummary,
    ModrinthCatalog,
};
use mh_core::instance::ModpackSource;
use mh_core::{ErrorCode, GamePort, MemoryLimits, ServerName};
use mh_net::{HostAllowlist, HttpClient, NetConfig};
use mh_platform::DataDirs;
use mh_providers::Modrinth;
use mh_store::{ContentManifest, ContentSource, ProjectId, sha1_hex};
use mh_supervisor::SupervisorConfig;
use time::macros::datetime;
use tokio::net::TcpListener;

use support::{
    FakeJava, FakeLoaderCatalog, PackTestCatalog, finish, make_app, make_mrpack, modrinth_req,
    pack_entry, server_root, spawn_stub_server, ver_json,
};

#[tokio::test]
async fn installs_50_file_pack_skips_client_writes_content_toml_and_summary() {
    let mut cat = PackTestCatalog {
        delay: Duration::from_millis(8),
        ..Default::default()
    };
    let mut index_files = Vec::new();
    for i in 0..50 {
        let bytes = format!("mod-payload-{i}").into_bytes();
        let url = format!("https://cdn.modrinth.com/data/proj{i}/versions/ver{i}/mod-{i:02}.jar");
        let path = format!("mods/mod-{i:02}.jar");
        index_files.push(pack_entry(&path, &url, &bytes, Some("required")));
        cat.files.insert(url, bytes);
    }
    for i in 0..2 {
        let bytes = format!("client-only-{i}").into_bytes();
        let url = format!("https://cdn.modrinth.com/data/cli{i}/versions/vc{i}/client-{i}.jar");
        index_files.push(pack_entry(
            &format!("mods/client-{i}.jar"),
            &url,
            &bytes,
            Some("unsupported"),
        ));
        cat.files.insert(url, bytes);
    }
    let mrpack = make_mrpack("Goi 50 Mod", &index_files, &[]);
    cat.add_pack_version(
        "pack-50",
        "v50-1",
        "1.0.0",
        "https://cdn.modrinth.com/data/pack-50/versions/v50-1/p.mrpack",
        mrpack,
    );
    let cat = Arc::new(cat);
    let (tmp, app) = make_app(16 * 1024, cat.clone());

    let created = app
        .create_from_modpack(modrinth_req("pack-50", Some("v50-1"), "Gói 50"))
        .unwrap();
    let snap = finish(&app, created.job_id).await;
    assert_eq!(snap.state, JobState::Succeeded, "{snap:?}");
    let summary: ModpackSummary = serde_json::from_value(snap.result.unwrap()).unwrap();
    assert_eq!(summary.installed_mods, 50);
    assert_eq!(
        summary.skipped_client,
        ["mods/client-0.jar", "mods/client-1.jar"]
    );
    assert!(summary.pending_curseforge.is_empty());

    let peak = cat.max_in_flight.load(Ordering::SeqCst);
    assert!((2..=6).contains(&peak), "độ song song bất thường: {peak}");

    let root = server_root(&tmp, created.server_id);
    let mods_dir = root.join("server/mods");
    for i in 0..50 {
        assert!(mods_dir.join(format!("mod-{i:02}.jar")).is_file());
    }
    assert!(!mods_dir.join("client-0.jar").exists());

    let manifest = ContentManifest::load(&root.join("content.toml")).unwrap();
    assert_eq!(manifest.file.len(), 50);
    let rec0 = manifest
        .file
        .iter()
        .find(|r| r.path == "mods/mod-00.jar")
        .unwrap();
    assert_eq!(rec0.source, ContentSource::Modpack);
    assert!(rec0.from_modpack);
    assert_eq!(rec0.project_id, Some(ProjectId::Str("proj0".to_owned())));
    assert_eq!(rec0.version_id.as_deref(), Some("ver0"));

    let spec = app.get(created.server_id).unwrap().spec.unwrap();
    let mp = spec.modpack.unwrap();
    assert_eq!(mp.source, ModpackSource::Modrinth);
    assert_eq!(mp.project_id.as_ref().map(|p| p.as_str()), Some("pack-50"));
    assert_eq!(mp.version_id.as_ref().map(|v| v.as_str()), Some("v50-1"));
    assert_eq!(mp.name.as_str(), "Goi 50 Mod");
    assert_eq!(mp.version_name.as_ref().map(|v| v.as_str()), Some("1.0.0"));
}

#[tokio::test]
async fn bad_checksum_fails_job_and_cleans_server_directory() {
    let mut cat = PackTestCatalog::default();
    let good = b"ok-jar";
    let bad_expected = b"expected-jar";
    let u1 = "https://cdn.modrinth.com/data/p1/versions/v1/ok.jar";
    let u2 = "https://cdn.modrinth.com/data/p2/versions/v2/bad.jar";
    let mrpack = make_mrpack(
        "Loi Hash",
        &[
            pack_entry("mods/ok.jar", u1, good, None),
            pack_entry("mods/bad.jar", u2, bad_expected, None),
        ],
        &[],
    );
    cat.files.insert(u1.to_owned(), good.to_vec());
    cat.files.insert(u2.to_owned(), b"tampered".to_vec());
    cat.add_pack_version(
        "bad-pack",
        "vb1",
        "1.0.0",
        "https://cdn.modrinth.com/p.mrpack",
        mrpack,
    );
    let (tmp, app) = make_app(8 * 1024, Arc::new(cat));

    let created = app
        .create_from_modpack(modrinth_req("bad-pack", Some("vb1"), "Hỏng"))
        .unwrap();
    let snap = finish(&app, created.job_id).await;
    assert!(matches!(
        snap.state,
        JobState::Failed {
            code: ErrorCode::ChecksumMismatch,
            ..
        }
    ));
    assert!(!server_root(&tmp, created.server_id).exists());
    assert!(
        !app.list()
            .unwrap()
            .iter()
            .any(|s| s.id == created.server_id)
    );
}

#[tokio::test]
async fn overrides_beat_downloads_and_server_overrides_win() {
    let mut cat = PackTestCatalog::default();
    let downloaded = b"from-cdn";
    let url = "https://cdn.modrinth.com/data/shared/versions/v9/shared.jar";
    cat.files.insert(url.to_owned(), downloaded.to_vec());
    let mrpack = make_mrpack(
        "Ghi de",
        &[pack_entry("mods/shared.jar", url, downloaded, None)],
        &[
            ("overrides/mods/shared.jar", b"from-overrides"),
            ("overrides/config/a.txt", b"override-a"),
            ("server-overrides/config/a.txt", b"server-a"),
            ("server-overrides/config/only-srv.txt", b"srv-only"),
        ],
    );
    cat.add_pack_version(
        "ov-pack",
        "ov1",
        "2.0.0",
        "https://cdn.modrinth.com/ov.mrpack",
        mrpack,
    );
    let (tmp, app) = make_app(8 * 1024, Arc::new(cat));

    let created = app
        .create_from_modpack(modrinth_req("ov-pack", None, "Override"))
        .unwrap();
    assert_eq!(
        finish(&app, created.job_id).await.state,
        JobState::Succeeded
    );

    let root = server_root(&tmp, created.server_id);
    assert_eq!(
        std::fs::read(root.join("server/mods/shared.jar")).unwrap(),
        b"from-overrides"
    );
    assert_eq!(
        std::fs::read(root.join("server/config/a.txt")).unwrap(),
        b"server-a"
    );
    assert_eq!(
        std::fs::read(root.join("server/config/only-srv.txt")).unwrap(),
        b"srv-only"
    );

    let manifest = ContentManifest::load(&root.join("content.toml")).unwrap();
    let rec = manifest
        .file
        .iter()
        .find(|r| r.path == "mods/shared.jar")
        .unwrap();
    assert_eq!(rec.sha1, sha1_hex(b"from-overrides"));
    assert_eq!(rec.source, ContentSource::Modpack);
    assert_eq!(
        rec.project_id, None,
        "file bị override đè không được giữ ID bản CDN"
    );
}

#[tokio::test]
async fn modrinth_catalog_rerun_reuses_cache_without_redownloading() {
    let mod_bytes = b"cached-mod-jar".to_vec();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mod_url = format!("http://127.0.0.1:{port}/mods/cached.jar");
    let mrpack = make_mrpack(
        "Cache Pack",
        &[pack_entry("mods/cached.jar", &mod_url, &mod_bytes, None)],
        &[],
    );
    let pack_url = format!("http://127.0.0.1:{port}/pack/p.mrpack");
    let newer_no_pack = ver_json(
        "v-zip",
        "cool",
        datetime!(2025-02-01 0:00 UTC),
        &mod_url,
        "other.zip",
        &mod_bytes,
    );
    let picked = ver_json(
        "v-pack",
        "cool",
        datetime!(2025-01-15 0:00 UTC),
        &pack_url,
        "cool.mrpack",
        &mrpack,
    );
    let version_body = serde_json::to_vec(&picked).unwrap();
    let versions_body = serde_json::to_vec(&[newer_no_pack, picked]).unwrap();

    let mut routes = HashMap::new();
    routes.insert(
        "/v2/project/cool/version".to_owned(),
        ("application/json", versions_body),
    );
    routes.insert(
        "/v2/version/v-pack".to_owned(),
        ("application/json", version_body),
    );
    routes.insert(
        "/pack/p.mrpack".to_owned(),
        ("application/octet-stream", mrpack),
    );
    routes.insert(
        "/mods/cached.jar".to_owned(),
        ("application/java-archive", mod_bytes),
    );
    let counts = spawn_stub_server(listener, routes).await;

    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let http = Arc::new(HttpClient::new(NetConfig::new(dirs.cache())).unwrap());
    let hosts = HostAllowlist::new(&["127.0.0.1"]);
    let mr = Modrinth::with_base_url(Arc::clone(&http), format!("http://127.0.0.1:{port}/v2"))
        .with_hosts(hosts);
    let catalog = Arc::new(ModrinthCatalog::new(http).with_modrinth(mr, hosts));
    let deps = AppDeps {
        catalog: Arc::new(FakeLoaderCatalog),
        java: Arc::new(FakeJava),
        port_free: Arc::new(|_| true),
        free_space: Arc::new(|_| Some(100 * 1024 * 1024 * 1024)),
        total_memory_mb: 16 * 1024,
        supervisor: SupervisorConfig::default(),
        save_flush_timeout: Duration::from_secs(60),
        metrics: MetricsConfig::default(),
    };
    let app = Arc::new(App::with_deps(dirs, deps).unwrap().with_content(catalog));

    let j1 = app
        .create_from_modpack(modrinth_req("cool", None, "Lần 1"))
        .unwrap()
        .job_id;
    assert_eq!(finish(&app, j1).await.state, JobState::Succeeded);
    let j2 = app
        .create_from_modpack(ModpackRequest {
            port: Some(GamePort::new(25566).unwrap()),
            ..modrinth_req("cool", Some("v-pack"), "Lần 2")
        })
        .unwrap()
        .job_id;
    assert_eq!(finish(&app, j2).await.state, JobState::Succeeded);

    let map = counts.lock().unwrap();
    assert_eq!(map.get("/pack/p.mrpack"), Some(&1));
    assert_eq!(map.get("/mods/cached.jar"), Some(&1));
}

#[tokio::test]
async fn suggested_ram_and_uploaded_mrpack_and_zip_rejection() {
    for (host_gb, expected_max) in [(8u64, 4096u32), (16, 8192), (32, 10240)] {
        let mut cat = PackTestCatalog::default();
        let mrpack = make_mrpack("RAM Pack", &[], &[]);
        cat.add_pack_version(
            "ram",
            "rv1",
            "1.0.0",
            "https://cdn.modrinth.com/r.mrpack",
            mrpack,
        );
        let (_tmp, app) = make_app(host_gb * 1024, Arc::new(cat));
        assert_eq!(app.suggested_modpack_memory().max_mb(), expected_max);
        let c = app
            .create_from_modpack(modrinth_req("ram", Some("rv1"), "RAM"))
            .unwrap();
        assert_eq!(finish(&app, c.job_id).await.state, JobState::Succeeded);
        assert_eq!(
            app.get(c.server_id)
                .unwrap()
                .spec
                .unwrap()
                .runtime
                .memory_mb
                .max_mb(),
            expected_max
        );
    }

    let (tmp, app) = make_app(16 * 1024, Arc::new(PackTestCatalog::default()));
    let mrpack_file = tmp.path().join("upload.mrpack");
    std::fs::write(
        &mrpack_file,
        make_mrpack("Upload Pack", &[], &[("overrides/config/u.txt", b"ok")]),
    )
    .unwrap();
    let c1 = app
        .create_from_modpack(ModpackRequest {
            source: ModpackSourceInput::UploadedFile {
                path: mrpack_file.clone(),
                original_name: "upload.mrpack".to_owned(),
            },
            name: ServerName::parse("Tải lên").unwrap(),
            memory_mb: Some(MemoryLimits::new(2048, 6144).unwrap()),
            port: None,
        })
        .unwrap();
    assert_eq!(finish(&app, c1.job_id).await.state, JobState::Succeeded);
    assert!(!mrpack_file.exists());
    assert_eq!(
        app.get(c1.server_id)
            .unwrap()
            .spec
            .unwrap()
            .modpack
            .unwrap()
            .source,
        ModpackSource::File
    );

    let bad_zip = tmp.path().join("cf.zip");
    std::fs::write(&bad_zip, b"not-a-mrpack").unwrap();
    let err = app
        .create_from_modpack(ModpackRequest {
            source: ModpackSourceInput::UploadedFile {
                path: bad_zip.clone(),
                original_name: "cf.zip".to_owned(),
            },
            name: ServerName::parse("Zip lỗi").unwrap(),
            memory_mb: None,
            port: Some(GamePort::new(25567).unwrap()),
        })
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::ModpackInvalid);
    assert!(!bad_zip.exists());

    let corrupt_pack = tmp.path().join("corrupt.mrpack");
    std::fs::write(&corrupt_pack, b"not-a-zip").unwrap();
    let c2 = app
        .create_from_modpack(ModpackRequest {
            source: ModpackSourceInput::UploadedFile {
                path: corrupt_pack.clone(),
                original_name: "corrupt.mrpack".to_owned(),
            },
            name: ServerName::parse("Hỏng").unwrap(),
            memory_mb: None,
            port: Some(GamePort::new(25567).unwrap()),
        })
        .unwrap();
    assert!(matches!(
        finish(&app, c2.job_id).await.state,
        JobState::Failed {
            code: ErrorCode::ModpackInvalid,
            ..
        }
    ));
    assert!(!corrupt_pack.exists());
    assert!(!server_root(&tmp, c2.server_id).exists());
}
