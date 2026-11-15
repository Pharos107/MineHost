//! Cập nhật modpack (spec P3, mục 3.9 và tiêu chí 4): file của bản cũ vào thùng rác, mod người
//! dùng thêm vẫn còn, có bản `before_update`, khôi phục bản đó thì về đúng bản pack cũ.

use std::sync::Arc;
use std::time::Duration;

use mh_app::{App, BackupKind, JobState};
use mh_core::{ServerId, ServerState};
use mh_store::ContentManifest;

use super::support::{
    PackTestCatalog, RunnableJava, finish, make_app_with_java, make_mrpack_for, modrinth_req,
    pack_entry, server_root,
};

const PACK: &str = "pack-up";

fn add_pack(cat: &mut PackTestCatalog, vid: &str, fabric: &str, mods: &[(&str, &str)], cfg: &str) {
    let mut entries = Vec::new();
    for (name, payload) in mods {
        let url = format!("https://cdn.modrinth.com/data/{name}/versions/{vid}/{name}.jar");
        let bytes = payload.as_bytes().to_vec();
        entries.push(pack_entry(
            &format!("mods/{name}.jar"),
            &url,
            &bytes,
            Some("required"),
        ));
        cat.files.insert(url, bytes);
    }
    let mrpack = make_mrpack_for(
        "Gói cập nhật",
        fabric,
        &entries,
        &[("overrides/config/pack.txt", cfg.as_bytes())],
    );
    let url = format!("https://cdn.modrinth.com/data/{PACK}/versions/{vid}/{vid}.mrpack");
    cat.add_pack_version(PACK, vid, vid, &url, mrpack);
}

async fn installed_v1() -> (tempfile::TempDir, Arc<App>, ServerId) {
    let mut cat = PackTestCatalog::default();
    add_pack(
        &mut cat,
        "v1",
        "0.16.9",
        &[("a1", "A bản 1"), ("b", "B")],
        "v1",
    );
    add_pack(&mut cat, "v2", "0.16.10", &[("a2", "A bản 2")], "v2");
    let (tmp, app) = make_app_with_java(16 * 1024, Arc::new(cat), Arc::new(RunnableJava));
    let created = app
        .create_from_modpack(modrinth_req(PACK, Some("v1"), "Pack"))
        .unwrap();
    let snap = finish(&app, created.job_id).await;
    assert_eq!(snap.state, JobState::Succeeded, "{snap:?}");
    let id = created.server_id;
    let mods = server_root(&tmp, id).join("server/mods");
    std::fs::write(mods.join("user.jar"), b"mod nguoi dung").unwrap();
    (tmp, app, id)
}

#[tokio::test]
async fn update_replaces_pack_files_keeps_user_mods_and_restore_goes_back() {
    let (tmp, app, id) = installed_v1().await;
    let root = server_root(&tmp, id);
    let job = app.update_modpack(id, "v2").unwrap();
    let snap = finish(&app, job).await;
    assert_eq!(snap.state, JobState::Succeeded, "{snap:?}");
    let result = snap.result.unwrap();
    assert_eq!(result["removed"], 2, "{result}");
    assert_eq!(result["kept"], 1, "{result}");

    let mods = root.join("server/mods");
    assert!(mods.join("a2.jar").is_file());
    assert!(mods.join("user.jar").is_file());
    assert!(!mods.join("a1.jar").exists() && !mods.join("b.jar").exists());
    let cfg = std::fs::read_to_string(root.join("server/config/pack.txt")).unwrap();
    assert_eq!(cfg, "v2");
    let spec = app.get(id).unwrap().spec.unwrap();
    assert_eq!(
        spec.modpack
            .as_ref()
            .unwrap()
            .version_id
            .as_ref()
            .unwrap()
            .as_str(),
        "v2"
    );
    assert_eq!(
        spec.game.loader_version.as_ref().unwrap().as_str(),
        "0.16.10"
    );
    let manifest = ContentManifest::load(&root.join("content.toml")).unwrap();
    assert!(
        manifest
            .file
            .iter()
            .any(|r| r.path == "mods/a2.jar" && r.from_modpack)
    );
    assert!(
        manifest.file.iter().all(|r| r.path != "mods/user.jar"),
        "mod thêm tay giữ là thêm tay"
    );

    let backups = app.list_backups(id).unwrap();
    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].kind, BackupKind::BeforeUpdate);
    assert_eq!(result["backup_id"], backups[0].id.as_str());

    // Tiêu chí 4: khôi phục bản `before_update` đưa server về bản pack cũ.
    let restored = app.restore(id, &backups[0].id).unwrap();
    assert_eq!(
        finish(&app, restored.job_id).await.state,
        JobState::Succeeded
    );
    assert!(mods.join("a1.jar").is_file() && mods.join("b.jar").is_file());
    assert!(!mods.join("a2.jar").exists());
    let spec = app.get(id).unwrap().spec.unwrap();
    assert_eq!(
        spec.modpack
            .as_ref()
            .unwrap()
            .version_id
            .as_ref()
            .unwrap()
            .as_str(),
        "v1"
    );
    assert_eq!(
        spec.game.loader_version.as_ref().unwrap().as_str(),
        "0.16.9"
    );
    let manifest = ContentManifest::load(&root.join("content.toml")).unwrap();
    assert!(
        manifest
            .file
            .iter()
            .any(|r| r.path == "mods/a1.jar" && r.from_modpack)
    );
    let report = app.preflight(id).await.unwrap();
    assert!(
        report.missing_deps.is_empty() && report.pending.is_empty(),
        "{report:?}"
    );

    // ... và bật được: `fake-mc` đóng vai Java, server lên tới `Running` rồi tắt.
    app.accept_eula(id).unwrap();
    app.start(id).await.unwrap();
    let status = app
        .handle(id)
        .unwrap()
        .wait_for(Duration::from_secs(20), |s| s.state == ServerState::Running)
        .await;
    assert_eq!(status.state, ServerState::Running, "{status:?}");
    app.stop(id).await.unwrap();
}

#[tokio::test]
async fn only_modrinth_modpack_servers_can_be_updated() {
    let (_tmp, app, id) = installed_v1().await;
    let err = app.update_modpack(id, " ").unwrap_err();
    assert_eq!(err.code, mh_core::ErrorCode::InvalidInput);

    let job = app.update_modpack(id, "không-có").unwrap();
    let snap = finish(&app, job).await;
    assert!(matches!(snap.state, JobState::Failed { .. }), "{snap:?}");
    assert!(
        app.list_backups(id).unwrap().is_empty(),
        "lỗi trước khi sao lưu thì không có backup"
    );
}
