//! Tích hợp luồng cài modpack CurseForge trong `mh-app` (API, server pack lồng 1 tầng,
//! file cấm phân phối `[[pending]]`, `.zip` tải lên, mod chỉ chạy client và thiếu API key).

#![allow(clippy::unwrap_used, clippy::panic)]

mod server_starter;
mod support;

use std::collections::HashMap;
use std::sync::atomic::Ordering;

use mh_app::{JobState, ModpackRequest, ModpackSourceInput, ModpackSummary};
use mh_core::instance::ModpackSource;
use mh_core::{ErrorCode, GamePort, ServerName};
use mh_store::{ContentManifest, ContentSource, ProjectId, ServerStore};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use support::{
    build_jar, build_zip, cf_app, cf_manifest, file_json, finish, mod_json, mount_batch_files,
    mount_mod, sha1_hex,
};

#[tokio::test]
async fn curseforge_api_modpack_with_restricted_mod_client_jar_and_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let fapi_jar = build_jar("*", "fapi");
    let client_jar = build_jar("client", "iris-client");
    let blocked_sha1 = "c".repeat(40);

    let manifest = cf_manifest(
        "CF Pending Pack",
        "1.2.3",
        &[(306612, 5800000, true), (999001, 5800999, true)],
        "overrides",
    );
    let pack_zip = build_zip(&[
        ("manifest.json", &manifest),
        ("overrides/mods/iris-client.jar", &client_jar),
        ("overrides/config/cf.cfg", b"cfg-ok"),
    ]);

    let pack_file = file_json(
        5900001,
        910001,
        "cf-pack.zip",
        Some(format!("{base}/cdn/pack.zip")),
        &sha1_hex(&pack_zip),
        pack_zip.len(),
        None,
        false,
    );
    let fapi_file = file_json(
        5800000,
        306612,
        "fapi-0.110.jar",
        Some(format!("{base}/cdn/fapi.jar")),
        &sha1_hex(&fapi_jar),
        fapi_jar.len(),
        None,
        false,
    );
    let blocked_file = file_json(
        5800999,
        999001,
        "restricted-1.0.jar",
        None,
        &blocked_sha1,
        2048,
        None,
        false,
    );

    mount_mod(
        &server,
        910001,
        "cf-pending-pack",
        true,
        vec![pack_file.clone()],
    )
    .await;
    mount_batch_files(
        &server,
        HashMap::from([
            (5900001, pack_file),
            (5800000, fapi_file),
            (5800999, blocked_file),
        ]),
        vec![
            mod_json(306612, "fabric-api", "Fabric API", 6, true),
            mod_json(999001, "restricted-mod", "Restricted Mod", 6, false),
        ],
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/cdn/pack.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(pack_zip))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/cdn/fapi.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(fapi_jar))
        .expect(1)
        .mount(&server)
        .await;

    let (tmp, app, _) = cf_app(&server, true);
    let req = ModpackRequest {
        source: ModpackSourceInput::Curseforge {
            project_id: "910001".to_owned(),
            version_id: Some("5900001".to_owned()),
        },
        name: ServerName::parse("CF Pending").unwrap(),
        memory_mb: None,
        port: Some(GamePort::new(25565).unwrap()),
    };
    let created = app.create_from_modpack(req.clone()).unwrap();
    let snap = finish(&app, created.job_id).await;
    assert_eq!(snap.state, JobState::Succeeded);

    let summary: ModpackSummary = serde_json::from_value(snap.result.unwrap()).unwrap();
    assert_eq!(summary.pending_curseforge, vec!["mods/restricted-1.0.jar"]);
    assert_eq!(summary.installed_mods, 2);

    let paths = ServerStore::new(tmp.path().join("servers"), tmp.path().join("trash"))
        .paths(created.server_id);
    assert!(paths.server_dir.join("mods/fapi-0.110.jar").is_file());
    assert!(
        paths
            .server_dir
            .join("mods-disabled/iris-client.jar")
            .is_file()
    );
    assert!(!paths.server_dir.join("mods/iris-client.jar").exists());
    assert_eq!(
        std::fs::read(paths.server_dir.join("config/cf.cfg")).unwrap(),
        b"cfg-ok"
    );

    let view = app.get(created.server_id).unwrap();
    let mp = view.spec.unwrap().modpack.unwrap();
    assert_eq!(mp.source, ModpackSource::Curseforge);
    assert_eq!(mp.project_id.unwrap().as_str(), "910001");
    assert_eq!(mp.version_id.unwrap().as_str(), "5900001");
    assert!(view.preflight_blocking);

    let cm = ContentManifest::load(&paths.content_file).unwrap();
    assert_eq!(cm.file.len(), 2);
    assert!(
        cm.file
            .iter()
            .all(|r| r.source == ContentSource::Modpack && r.from_modpack)
    );
    let fapi_rec = cm
        .file
        .iter()
        .find(|r| r.path == "mods/fapi-0.110.jar")
        .unwrap();
    assert_eq!(fapi_rec.project_id, Some(ProjectId::Int(306612)));
    assert_eq!(fapi_rec.version_id.as_deref(), Some("5800000"));
    assert_eq!(cm.pending.len(), 1);
    assert_eq!(cm.pending[0].path, "mods/restricted-1.0.jar");
    assert_eq!(cm.pending[0].sha1, blocked_sha1);
    assert!(
        cm.pending[0]
            .page_url
            .contains("/restricted-mod/download/5800999")
    );

    assert!(
        app.preflight(created.server_id)
            .await
            .unwrap()
            .is_blocking()
    );

    // Lần 2 dùng cùng HTTP cache theo SHA-1, không tải lại `/cdn/pack.zip` hay `/cdn/fapi.jar`.
    let c2 = app
        .create_from_modpack(ModpackRequest {
            port: Some(GamePort::new(25566).unwrap()),
            name: ServerName::parse("CF Cached").unwrap(),
            ..req
        })
        .unwrap();
    assert_eq!(finish(&app, c2.job_id).await.state, JobState::Succeeded);
}

#[tokio::test]
async fn curseforge_server_pack_strips_nested_root_skips_launchers_and_falls_back() {
    let server = MockServer::start().await;
    let base = server.uri();
    let fapi_jar = build_jar("*", "fapi");

    let client_manifest = cf_manifest("SP Pack", "1.0.0", &[(306612, 5800000, true)], "overrides");
    let client_zip = build_zip(&[
        ("manifest.json", &client_manifest),
        ("overrides/config/from-manifest.txt", b"fallback"),
    ]);
    let sp_zip = build_zip(&[
        ("CFServer-1.0/mods/fapi-0.110.jar", &fapi_jar),
        ("CFServer-1.0/config/srv.toml", b"ok = true"),
        ("CFServer-1.0/start.sh", b"#!/bin/sh"),
        ("CFServer-1.0/run.bat", b"java -jar"),
        ("CFServer-1.0/launch.cmd", b"echo hi"),
        ("CFServer-1.0/forge-1.21.4-47.2.0-installer.jar", b"inst"),
        ("CFServer-1.0/fabric-server-launch.jar", b"upstream-jar"),
        ("CFServer-1.0/libraries/net/fabric/lib.jar", b"lib"),
    ]);

    let ver_ok = file_json(
        5900002,
        910002,
        "sp-client.zip",
        Some(format!("{base}/cdn/client.zip")),
        &sha1_hex(&client_zip),
        client_zip.len(),
        Some(5900003),
        false,
    );
    let sp_ok = file_json(
        5900003,
        910002,
        "server-pack.zip",
        Some(format!("{base}/cdn/server-pack.zip")),
        &sha1_hex(&sp_zip),
        sp_zip.len(),
        None,
        true,
    );
    let ver_fb = file_json(
        5900004,
        910003,
        "fb-client.zip",
        Some(format!("{base}/cdn/client.zip")),
        &sha1_hex(&client_zip),
        client_zip.len(),
        Some(5900005),
        false,
    );
    let sp_blocked = file_json(
        5900005,
        910003,
        "blocked-sp.zip",
        None,
        &"d".repeat(40),
        1024,
        None,
        true,
    );
    let fapi_file = file_json(
        5800000,
        306612,
        "fapi-0.110.jar",
        Some(format!("{base}/cdn/fapi.jar")),
        &sha1_hex(&fapi_jar),
        fapi_jar.len(),
        None,
        false,
    );

    mount_mod(&server, 910002, "cf-sp", true, vec![ver_ok.clone()]).await;
    mount_mod(&server, 910003, "cf-sp-fb", true, vec![ver_fb.clone()]).await;
    mount_batch_files(
        &server,
        HashMap::from([
            (5900002, ver_ok),
            (5900003, sp_ok),
            (5900004, ver_fb),
            (5900005, sp_blocked),
            (5800000, fapi_file),
        ]),
        vec![
            mod_json(910002, "cf-sp", "SP", 4471, true),
            mod_json(910003, "cf-sp-fb", "FB", 4471, false),
            mod_json(306612, "fabric-api", "Fabric API", 6, true),
        ],
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/cdn/client.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(client_zip))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/cdn/server-pack.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(sp_zip))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/cdn/fapi.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(fapi_jar))
        .mount(&server)
        .await;

    let (tmp, app, loader) = cf_app(&server, true);
    let c1 = app
        .create_from_modpack(ModpackRequest {
            source: ModpackSourceInput::Curseforge {
                project_id: "910002".to_owned(),
                version_id: None,
            },
            name: ServerName::parse("Server Pack").unwrap(),
            memory_mb: None,
            port: Some(GamePort::new(25565).unwrap()),
        })
        .unwrap();
    assert_eq!(finish(&app, c1.job_id).await.state, JobState::Succeeded);
    assert_eq!(loader.plan_calls.load(Ordering::SeqCst), 1);

    let s1 = ServerStore::new(tmp.path().join("servers"), tmp.path().join("trash"))
        .paths(c1.server_id)
        .server_dir;
    assert!(s1.join("mods/fapi-0.110.jar").is_file());
    assert!(s1.join("config/srv.toml").is_file());
    assert_eq!(
        std::fs::read(s1.join("fabric-server-launch.jar")).unwrap(),
        b"loader-jar"
    );
    for skipped in [
        "start.sh",
        "run.bat",
        "launch.cmd",
        "forge-1.21.4-47.2.0-installer.jar",
        "libraries",
    ] {
        assert!(!s1.join(skipped).exists(), "{skipped} phải bị bỏ qua");
    }

    // Server pack cấm phân phối -> tự quay về dựng từ manifest client.
    let c2 = app
        .create_from_modpack(ModpackRequest {
            source: ModpackSourceInput::Curseforge {
                project_id: "910003".to_owned(),
                version_id: Some("5900004".to_owned()),
            },
            name: ServerName::parse("Fallback Manifest").unwrap(),
            memory_mb: None,
            port: Some(GamePort::new(25566).unwrap()),
        })
        .unwrap();
    assert_eq!(finish(&app, c2.job_id).await.state, JobState::Succeeded);
    let s2 = ServerStore::new(tmp.path().join("servers"), tmp.path().join("trash"))
        .paths(c2.server_id)
        .server_dir;
    assert!(s2.join("mods/fapi-0.110.jar").is_file());
    assert_eq!(
        std::fs::read(s2.join("config/from-manifest.txt")).unwrap(),
        b"fallback"
    );
}

#[tokio::test]
async fn curseforge_missing_key_uploaded_zip_and_invalid_zip() {
    let server = MockServer::start().await;
    let base = server.uri();
    let fapi_jar = build_jar("*", "fapi");
    let manifest = cf_manifest(
        "Upload CF",
        "1.0.0",
        &[(306612, 5800000, true)],
        "overrides",
    );
    let valid_zip = build_zip(&[
        ("manifest.json", &manifest),
        ("overrides/config/up.txt", b"from-zip"),
    ]);

    let (_tmp0, no_key, _) = cf_app(&server, false);
    let err = no_key
        .create_from_modpack(ModpackRequest {
            source: ModpackSourceInput::Curseforge {
                project_id: "910001".to_owned(),
                version_id: None,
            },
            name: ServerName::parse("No Key").unwrap(),
            memory_mb: None,
            port: Some(GamePort::new(25565).unwrap()),
        })
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::CurseforgeUnavailable);

    let zip_no_key = _tmp0.path().join("cf-no-key.zip");
    std::fs::write(&zip_no_key, &valid_zip).unwrap();
    let c_no_key = no_key
        .create_from_modpack(ModpackRequest {
            source: ModpackSourceInput::UploadedFile {
                path: zip_no_key.clone(),
                original_name: "cf.zip".to_owned(),
            },
            name: ServerName::parse("Zip No Key").unwrap(),
            memory_mb: None,
            port: Some(GamePort::new(25566).unwrap()),
        })
        .unwrap();
    let JobState::Failed { code, .. } = finish(&no_key, c_no_key.job_id).await.state else {
        panic!("phải lỗi thiếu key CurseForge");
    };
    assert_eq!(code, ErrorCode::CurseforgeUnavailable);
    assert!(!zip_no_key.exists());
    assert!(no_key.get(c_no_key.server_id).is_err());

    mount_batch_files(
        &server,
        HashMap::from([(
            5800000,
            file_json(
                5800000,
                306612,
                "fapi.jar",
                Some(format!("{base}/cdn/fapi.jar")),
                &sha1_hex(&fapi_jar),
                fapi_jar.len(),
                None,
                false,
            ),
        )]),
        vec![mod_json(306612, "fabric-api", "Fabric API", 6, true)],
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/cdn/fapi.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(fapi_jar))
        .mount(&server)
        .await;

    let (tmp, app, _) = cf_app(&server, true);
    let zip_ok = tmp.path().join("cf-ok.zip");
    std::fs::write(&zip_ok, &valid_zip).unwrap();
    let c_ok = app
        .create_from_modpack(ModpackRequest {
            source: ModpackSourceInput::UploadedFile {
                path: zip_ok.clone(),
                original_name: "cf-ok.zip".to_owned(),
            },
            name: ServerName::parse("Zip Ok").unwrap(),
            memory_mb: None,
            port: Some(GamePort::new(25567).unwrap()),
        })
        .unwrap();
    assert_eq!(finish(&app, c_ok.job_id).await.state, JobState::Succeeded);
    assert!(!zip_ok.exists());
    assert_eq!(
        app.get(c_ok.server_id)
            .unwrap()
            .spec
            .unwrap()
            .modpack
            .unwrap()
            .source,
        ModpackSource::File
    );

    let corrupt = tmp.path().join("corrupt.zip");
    std::fs::write(&corrupt, b"not-a-zip").unwrap();
    let c_bad = app
        .create_from_modpack(ModpackRequest {
            source: ModpackSourceInput::UploadedFile {
                path: corrupt.clone(),
                original_name: "corrupt.zip".to_owned(),
            },
            name: ServerName::parse("Zip Hỏng").unwrap(),
            memory_mb: None,
            port: Some(GamePort::new(25568).unwrap()),
        })
        .unwrap();
    let JobState::Failed { code, .. } = finish(&app, c_bad.job_id).await.state else {
        panic!("zip hỏng phải thất bại");
    };
    assert_eq!(code, ErrorCode::ModpackInvalid);
    assert!(!corrupt.exists());
}
