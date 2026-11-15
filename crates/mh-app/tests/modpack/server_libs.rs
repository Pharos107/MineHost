//! Thư viện bị `.mrpack` gắn cờ `env.server = "unsupported"` nhưng mod chạy trên server bắt buộc cần
//! thì vẫn được cài (spec P3, mục 3.5). Gặp ở Prominence II (`athena`, `s_lib`).

use std::io::{Cursor, Write};
use std::sync::Arc;

use mh_app::{JobState, ModpackSummary};
use mh_store::ContentManifest;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use super::support::{
    PackTestCatalog, finish, make_app, make_mrpack, modrinth_req, pack_entry, server_root,
};

fn fabric_jar(meta: &str) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("fabric.mod.json", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(meta.as_bytes()).unwrap();
    zip.finish().unwrap().into_inner()
}

struct Pack {
    cat: PackTestCatalog,
    files: Vec<serde_json::Value>,
}

impl Pack {
    fn add(&mut self, name: &str, meta: &str, server_env: &str) {
        let bytes = fabric_jar(meta);
        let url = format!("https://cdn.modrinth.com/data/{name}/versions/v1/{name}.jar");
        self.files.push(pack_entry(
            &format!("mods/{name}.jar"),
            &url,
            &bytes,
            Some(server_env),
        ));
        self.cat.files.insert(url, bytes);
    }
}

#[tokio::test]
async fn required_library_marked_unsupported_is_installed_anyway() {
    let mut pack = Pack {
        cat: PackTestCatalog::default(),
        files: Vec::new(),
    };
    // Mod server cần `athena`; `athena` lại cần `s_lib`; cả hai bị pack gắn cờ chỉ client.
    pack.add(
        "chipped",
        r#"{"id":"chipped","depends":{"athena":">=3.1.1"}}"#,
        "required",
    );
    pack.add(
        "athena",
        r#"{"id":"athena","version":"3.1.2","depends":{"s_lib":"*"}}"#,
        "unsupported",
    );
    pack.add(
        "s-lib",
        r#"{"id":"s_lib","version":"3.0.1"}"#,
        "unsupported",
    );
    // Mod chỉ client thật: không ai cần trên server nên vẫn bị bỏ qua.
    pack.add(
        "zoomify",
        r#"{"id":"zoomify","environment":"client"}"#,
        "unsupported",
    );
    // Thư viện chỉ client mà mod client cần: không kéo về.
    pack.add(
        "iris",
        r#"{"id":"iris","environment":"client"}"#,
        "unsupported",
    );
    let mut files = pack.files.clone();
    // Cùng một file ghi hai lần (Pokemon Elysium 1.2.1): chỉ cài một lần.
    files.push(files[0].clone());
    let mrpack = make_mrpack("Libs", &files, &[]);
    pack.cat.add_pack_version(
        "libs",
        "v1",
        "1.0.0",
        "https://cdn.modrinth.com/data/libs/versions/v1/p.mrpack",
        mrpack,
    );
    let (tmp, app) = make_app(16 * 1024, Arc::new(pack.cat));

    let created = app
        .create_from_modpack(modrinth_req("libs", Some("v1"), "Libs"))
        .unwrap();
    let snap = finish(&app, created.job_id).await;
    assert_eq!(snap.state, JobState::Succeeded, "{snap:?}");
    let summary: ModpackSummary = serde_json::from_value(snap.result.unwrap()).unwrap();
    assert_eq!(
        summary.skipped_client,
        ["mods/zoomify.jar", "mods/iris.jar"]
    );
    assert_eq!(summary.installed_mods, 3);

    let root = server_root(&tmp, created.server_id);
    let mods = root.join("server/mods");
    for name in ["chipped", "athena", "s-lib"] {
        assert!(mods.join(format!("{name}.jar")).is_file(), "thiếu {name}");
    }
    assert!(!mods.join("zoomify.jar").exists());
    assert!(!mods.join("iris.jar").exists());
    assert!(!root.join("server/.minehost-server-libs").exists());

    let manifest = ContentManifest::load(&root.join("content.toml")).unwrap();
    assert!(manifest.file.iter().any(|r| r.path == "mods/athena.jar"));
    let report = app.preflight(created.server_id).await.unwrap();
    assert!(report.missing_deps.is_empty(), "{:?}", report.missing_deps);
}

#[tokio::test]
async fn nothing_is_downloaded_when_no_dependency_is_missing() {
    let mut pack = Pack {
        cat: PackTestCatalog::default(),
        files: Vec::new(),
    };
    pack.add("core", r#"{"id":"core"}"#, "required");
    pack.add("shader", r#"{"id":"shader"}"#, "unsupported");
    let mrpack = make_mrpack("Plain", &pack.files, &[]);
    pack.cat.add_pack_version(
        "plain",
        "v1",
        "1.0.0",
        "https://cdn.modrinth.com/data/plain/versions/v1/p.mrpack",
        mrpack,
    );
    let (tmp, app) = make_app(16 * 1024, Arc::new(pack.cat));
    let created = app
        .create_from_modpack(modrinth_req("plain", Some("v1"), "Plain"))
        .unwrap();
    let snap = finish(&app, created.job_id).await;
    assert_eq!(snap.state, JobState::Succeeded, "{snap:?}");
    let summary: ModpackSummary = serde_json::from_value(snap.result.unwrap()).unwrap();
    assert_eq!(summary.skipped_client, ["mods/shader.jar"]);
    let root = server_root(&tmp, created.server_id);
    assert!(!root.join("server/mods/shader.jar").exists());
    assert!(!root.join("server/.minehost-server-libs").exists());
}

#[tokio::test]
async fn same_path_with_different_hash_rejects_pack() {
    let mut pack = Pack {
        cat: PackTestCatalog::default(),
        files: Vec::new(),
    };
    pack.add("a", r#"{"id":"a"}"#, "required");
    let mut other = pack.files[0].clone();
    other["hashes"]["sha512"] = serde_json::json!("ab".repeat(64));
    let mut files = pack.files.clone();
    files.push(other);
    let mrpack = make_mrpack("Clash", &files, &[]);
    pack.cat.add_pack_version(
        "clash",
        "v1",
        "1.0.0",
        "https://cdn.modrinth.com/data/clash/versions/v1/p.mrpack",
        mrpack,
    );
    let (_tmp, app) = make_app(16 * 1024, Arc::new(pack.cat));
    let created = app
        .create_from_modpack(modrinth_req("clash", Some("v1"), "Clash"))
        .unwrap();
    let snap = finish(&app, created.job_id).await;
    assert!(
        matches!(
            snap.state,
            JobState::Failed {
                code: mh_core::ErrorCode::ModpackInvalid,
                ..
            }
        ),
        "{snap:?}"
    );
}
