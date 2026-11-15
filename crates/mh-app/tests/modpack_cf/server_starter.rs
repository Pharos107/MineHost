//! Server pack kiểu ServerStarter (chỉ có script tải mod lúc chạy, như Craftoria 1.37.0) không được
//! dùng: MineHost không chạy script đó nên server sẽ không có mod. Phải dựng từ manifest.

use std::collections::HashMap;

use mh_app::{JobState, ModpackRequest, ModpackSourceInput, ModpackSummary};
use mh_core::{GamePort, ServerName};
use mh_store::ServerStore;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::support::{
    build_jar, build_zip, cf_app, cf_manifest, file_json, finish, mod_json, mount_batch_files,
    mount_mod, sha1_hex,
};

async fn serve(server: &MockServer, url_path: &str, body: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path(url_path.to_owned()))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(server)
        .await;
}

#[tokio::test]
async fn server_pack_without_mods_falls_back_to_manifest() {
    let server = MockServer::start().await;
    let base = server.uri();
    let fapi_jar = build_jar("*", "fapi");
    let manifest = cf_manifest("Starter", "1.0.0", &[(306612, 5800000, true)], "overrides");
    let client_zip = build_zip(&[
        ("manifest.json", &manifest),
        ("overrides/config/client.txt", b"manifest"),
    ]);
    let starter_zip = build_zip(&[
        ("startserver.sh", b"#!/bin/sh\njava -jar serverstarter.jar"),
        ("startserver.bat", b"java -jar serverstarter.jar"),
        ("server-setup-config.yaml", b"modpack:\n  name: Starter\n"),
        ("README.md", b"chay startserver.sh"),
    ]);

    let client = file_json(
        5910001,
        920001,
        "starter-client.zip",
        Some(format!("{base}/cdn/client.zip")),
        &sha1_hex(&client_zip),
        client_zip.len(),
        Some(5910002),
        false,
    );
    let starter = file_json(
        5910002,
        920001,
        "starter-server.zip",
        Some(format!("{base}/cdn/starter.zip")),
        &sha1_hex(&starter_zip),
        starter_zip.len(),
        None,
        true,
    );
    let fapi = file_json(
        5800000,
        306612,
        "fapi-0.110.jar",
        Some(format!("{base}/cdn/fapi.jar")),
        &sha1_hex(&fapi_jar),
        fapi_jar.len(),
        None,
        false,
    );
    mount_mod(&server, 920001, "starter", true, vec![client.clone()]).await;
    mount_batch_files(
        &server,
        HashMap::from([(5910001, client), (5910002, starter), (5800000, fapi)]),
        vec![
            mod_json(920001, "starter", "Starter", 4471, true),
            mod_json(306612, "fabric-api", "Fabric API", 6, true),
        ],
    )
    .await;
    serve(&server, "/cdn/client.zip", client_zip).await;
    serve(&server, "/cdn/starter.zip", starter_zip).await;
    serve(&server, "/cdn/fapi.jar", fapi_jar).await;

    let (tmp, app, _loader) = cf_app(&server, true);
    let created = app
        .create_from_modpack(ModpackRequest {
            source: ModpackSourceInput::Curseforge {
                project_id: "920001".to_owned(),
                version_id: None,
            },
            name: ServerName::parse("Starter").unwrap(),
            memory_mb: None,
            port: Some(GamePort::new(25567).unwrap()),
        })
        .unwrap();
    let snap = finish(&app, created.job_id).await;
    assert_eq!(snap.state, JobState::Succeeded, "{snap:?}");
    let summary: ModpackSummary = serde_json::from_value(snap.result.unwrap()).unwrap();
    assert_eq!(summary.installed_mods, 1);

    let dir = ServerStore::new(tmp.path().join("servers"), tmp.path().join("trash"))
        .paths(created.server_id)
        .server_dir;
    assert!(dir.join("mods/fapi-0.110.jar").is_file());
    assert_eq!(
        std::fs::read(dir.join("config/client.txt")).unwrap(),
        b"manifest"
    );
    assert!(!dir.join("server-setup-config.yaml").exists());
}
