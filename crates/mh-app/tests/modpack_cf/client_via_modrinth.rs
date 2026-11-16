//! Pack CurseForge dựng từ `manifest.json`: mod không tự khai báo chỉ chạy client nhưng Modrinth ghi
//! `server_side = unsupported` thì vào `mods-disabled/` (spec P3, mục 3.5, nguồn 4).

use std::collections::HashMap;

use mh_app::{JobState, ModpackRequest, ModpackSourceInput, ModpackSummary};
use mh_core::{GamePort, ServerName};
use mh_store::{ContentManifest, ServerStore};
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::support::{
    build_jar, build_zip, cf_app, cf_manifest, file_json, finish, mod_json, mount_batch_files,
    mount_mod, sha1_hex,
};

fn modrinth_fixture(name: &str) -> Value {
    let p = format!(
        "{}/../mh-providers/fixtures/modrinth/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// Phiên bản Modrinth giả thuộc `project_id` (lấy khung từ fixture thật).
fn modrinth_version(project_id: &str) -> Value {
    let mut ver = modrinth_fixture("version_SMxNOGZ6.json");
    ver["project_id"] = json!(project_id);
    ver
}

fn modrinth_project(id: &str, server_side: &str) -> Value {
    let mut project = modrinth_fixture("project_sodium.json");
    project["id"] = json!(id);
    project["server_side"] = json!(server_side);
    project
}

#[tokio::test]
async fn manifest_mod_unsupported_on_modrinth_server_is_disabled() {
    let server = MockServer::start().await;
    let base = server.uri();
    let fapi_jar = build_jar("*", "fapi");
    let media_jar = build_jar("*", "watermedia");
    let blocked_sha1 = "d".repeat(40);

    let manifest = cf_manifest(
        "CF Client Pack",
        "1.0.0",
        &[
            (306612, 5800000, true),
            (869524, 5800001, true),
            (999001, 5800999, true),
            (999002, 5801000, true),
        ],
        "overrides",
    );
    let pack_zip = build_zip(&[("manifest.json", &manifest)]);
    let pack_file = file_json(
        5900002,
        910002,
        "cf-client-pack.zip",
        Some(format!("{base}/cdn/pack.zip")),
        &sha1_hex(&pack_zip),
        pack_zip.len(),
        None,
        false,
    );
    let file = |fid, pid, name: &str, url: &str, jar: &[u8]| {
        file_json(
            fid,
            pid,
            name,
            Some(format!("{base}{url}")),
            &sha1_hex(jar),
            jar.len(),
            None,
            false,
        )
    };
    let fapi_file = file(5800000, 306612, "fapi.jar", "/cdn/fapi.jar", &fapi_jar);
    let media_file = file(
        5800001,
        869524,
        "watermedia.jar",
        "/cdn/media.jar",
        &media_jar,
    );
    let blocked_file = file_json(
        5800999,
        999001,
        "restricted-client.jar",
        None,
        &blocked_sha1,
        2048,
        None,
        false,
    );
    // Resource pack bị xếp vào mục mod trên CurseForge: cấm phân phối nhưng server không nạp `.zip`.
    let zip_file = file_json(
        5801000,
        999002,
        "Pretty Textures.zip",
        None,
        &"e".repeat(40),
        4096,
        None,
        false,
    );
    mount_mod(
        &server,
        910002,
        "cf-client-pack",
        true,
        vec![pack_file.clone()],
    )
    .await;
    mount_batch_files(
        &server,
        HashMap::from([
            (5900002, pack_file),
            (5800000, fapi_file),
            (5800001, media_file),
            (5800999, blocked_file),
            (5801000, zip_file),
        ]),
        vec![
            mod_json(306612, "fabric-api", "Fabric API", 6, true),
            mod_json(869524, "watermedia", "WATERMeDIA", 6, true),
            mod_json(999001, "restricted-client", "Restricted Client", 6, false),
            mod_json(999002, "pretty-textures", "Pretty Textures", 6, false),
        ],
    )
    .await;
    for (url, bytes) in [
        ("/cdn/pack.zip", pack_zip),
        ("/cdn/fapi.jar", fapi_jar.clone()),
        ("/cdn/media.jar", media_jar.clone()),
    ] {
        Mock::given(method("GET"))
            .and(path(url))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
            .mount(&server)
            .await;
    }
    // Fabric API có trên Modrinth nhưng chạy được ở server; WATERMeDIA và mod cấm phân phối thì không.
    Mock::given(method("POST"))
        .and(path("/v2/version_files"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(Value::Object(
                [
                    (sha1_hex(&fapi_jar), modrinth_version("P7dR8mSH")),
                    (sha1_hex(&media_jar), modrinth_version("wmedia01")),
                    (blocked_sha1.clone(), modrinth_version("rclient1")),
                ]
                .into_iter()
                .collect(),
            )),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            modrinth_project("P7dR8mSH", "required"),
            modrinth_project("wmedia01", "unsupported"),
            modrinth_project("rclient1", "unsupported"),
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let (tmp, app, _) = cf_app(&server, true);
    let created = app
        .create_from_modpack(ModpackRequest {
            source: ModpackSourceInput::Curseforge {
                project_id: "910002".to_owned(),
                version_id: Some("5900002".to_owned()),
            },
            name: ServerName::parse("CF Client").unwrap(),
            memory_mb: None,
            port: Some(GamePort::new(25565).unwrap()),
        })
        .unwrap();
    let snap = finish(&app, created.job_id).await;
    assert_eq!(snap.state, JobState::Succeeded, "{:?}", snap.result);
    let summary: ModpackSummary = serde_json::from_value(snap.result.unwrap()).unwrap();
    assert_eq!(summary.skipped_client, ["mods/Pretty Textures.zip"]);
    assert_eq!(summary.pending_curseforge, ["mods/restricted-client.jar"]);

    let paths = ServerStore::new(tmp.path().join("servers"), tmp.path().join("trash"))
        .paths(created.server_id);
    assert!(paths.server_dir.join("mods/fapi.jar").is_file());
    assert!(
        paths
            .server_dir
            .join("mods-disabled/watermedia.jar")
            .is_file()
    );
    assert!(!paths.server_dir.join("mods/watermedia.jar").exists());

    let cm = ContentManifest::load(&paths.content_file).unwrap();
    let rec = |name: &str| {
        cm.file
            .iter()
            .find(|r| r.path.ends_with(name))
            .unwrap_or_else(|| panic!("thiếu bản ghi {name}: {:?}", cm.file))
    };
    assert!(rec("/watermedia.jar").client_only);
    assert!(!rec("/fapi.jar").client_only);
    assert_eq!(cm.pending.len(), 1);
    assert!(cm.pending[0].client_only);
}
