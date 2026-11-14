//! Kiểm thử đọc `.zip` modpack CurseForge, `resolve_curseforge_files` và server pack.

#![allow(clippy::unwrap_used)]

use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use std::sync::Arc;

use mh_core::{ErrorCode, LoaderKind};
use mh_net::{HostAllowlist, HttpClient, NetConfig};
use mh_providers::{
    CurseForge, PackLimits, extract_curseforge_overrides_with, extract_curseforge_server_pack,
    extract_curseforge_server_pack_with, inspect_curseforge_pack_with, plan_curseforge_pack_with,
};
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

const SHA1_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SHA1_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn test_http(dir: &Path) -> Arc<HttpClient> {
    Arc::new(HttpClient::new(NetConfig::new(dir.join("cache"))).unwrap())
}

fn build_zip(write: impl FnOnce(&mut ZipWriter<Cursor<Vec<u8>>>)) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    write(&mut writer);
    writer.finish().unwrap().into_inner()
}

fn add_file(w: &mut ZipWriter<Cursor<Vec<u8>>>, name: &str, bytes: &[u8]) {
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    w.start_file(name, opts).unwrap();
    w.write_all(bytes).unwrap();
}

fn write_zip(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let p = dir.join(name);
    fs::write(&p, bytes).unwrap();
    p
}

fn sample_manifest(overrides: &str, files: serde_json::Value) -> Vec<u8> {
    json!({
        "minecraft": {
            "version": "1.20.1",
            "modLoaders": [{ "id": "forge-47.2.0", "primary": true }]
        },
        "manifestType": "minecraftModpack",
        "manifestVersion": 1,
        "name": "CF Example Pack",
        "version": "2.0.0",
        "files": files,
        "overrides": overrides
    })
    .to_string()
    .into_bytes()
}

/// Dữ liệu giả header zip nói dối kích thước (copy kỹ thuật test từ PR-22).
fn patch_zip_uncompressed_size(buf: &mut [u8], real_size: u32, fake_size: u32) {
    let needle = real_size.to_le_bytes();
    let replacement = fake_size.to_le_bytes();
    for i in 0..=buf.len() - 4 {
        if buf[i..i + 4] == needle {
            buf[i..i + 4].copy_from_slice(&replacement);
        }
    }
}

#[tokio::test]
async fn plans_curseforge_pack_and_extracts_overrides() {
    let tmp = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    let mod_url = format!("{}/edge/238222/jei.jar", server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {
                    "id": 4912345,
                    "modId": 238222,
                    "displayName": "JEI 15.3",
                    "fileName": "jei-1.20.1-forge-15.3.jar",
                    "releaseType": 1,
                    "fileDate": "2026-01-10T00:00:00Z",
                    "fileLength": 500,
                    "downloadUrl": mod_url,
                    "hashes": [{ "value": SHA1_A, "algo": 1 }]
                },
                {
                    "id": 5800999,
                    "modId": 999001,
                    "displayName": "Restricted Mod 1.0",
                    "fileName": "restricted-mod-1.0.jar",
                    "releaseType": 1,
                    "fileDate": "2026-01-11T00:00:00Z",
                    "fileLength": 300,
                    "downloadUrl": null,
                    "hashes": [{ "value": SHA1_B, "algo": 1 }]
                }
            ]
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                { "id": 238222, "name": "JEI", "slug": "jei", "classId": 6, "allowModDistribution": true },
                { "id": 999001, "name": "Restricted", "slug": "restricted-mod", "classId": 6, "allowModDistribution": false }
            ]
        })))
        .mount(&server)
        .await;

    let manifest = sample_manifest(
        "overrides",
        json!([
            { "projectID": 238222, "fileID": 4912345, "required": true },
            { "projectID": 999001, "fileID": 5800999, "required": true }
        ]),
    );
    let zip_bytes = build_zip(|w| {
        add_file(w, "manifest.json", &manifest);
        add_file(w, "overrides/config/jei.toml", b"cheat = false");
        add_file(w, "ignored-other/foo.txt", b"skip");
    });
    let pack = write_zip(tmp.path(), "cf.zip", &zip_bytes);

    let hosts = HostAllowlist::new(&["127.0.0.1", "localhost"]);
    let cf = CurseForge::with_base_url(
        test_http(tmp.path()),
        "test-key",
        format!("{}/v1", server.uri()),
    )
    .with_hosts(hosts);

    let plan = plan_curseforge_pack_with(&pack, &cf, PackLimits::default())
        .await
        .unwrap();
    assert_eq!(plan.name, "CF Example Pack");
    assert_eq!(plan.version_id, "2.0.0");
    assert_eq!(plan.loader, LoaderKind::Forge);
    assert_eq!(plan.minecraft.as_str(), "1.20.1");
    assert_eq!(plan.loader_version.as_ref().unwrap().as_str(), "47.2.0");

    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.files[0].path, "mods/jei-1.20.1-forge-15.3.jar");
    assert_eq!(plan.files[0].cf_ids, Some((238222, 4912345)));

    assert_eq!(plan.blocked.len(), 1);
    assert_eq!(plan.blocked[0].path, "mods/restricted-mod-1.0.jar");
    assert_eq!(plan.blocked[0].project_id, 999001);
    assert_eq!(plan.blocked[0].file_id, 5800999);
    assert_eq!(plan.blocked[0].sha1, SHA1_B);
    assert_eq!(
        plan.blocked[0].page_url,
        "https://www.curseforge.com/minecraft/mc-mods/restricted-mod/download/5800999"
    );

    let dest = tmp.path().join("server");
    assert!(!dest.exists());
    extract_curseforge_overrides_with(&pack, &dest, PackLimits::default()).unwrap();
    assert_eq!(
        fs::read_to_string(dest.join("config/jei.toml")).unwrap(),
        "cheat = false"
    );
    assert!(!dest.join("foo.txt").exists());
}

#[test]
fn rejects_unsafe_overrides_symlink_manifest_and_zip_bomb() {
    let tmp = tempfile::tempdir().unwrap();

    // 1. manifest.overrides chứa `../`
    let bad_manifest = sample_manifest("../evil", json!([]));
    let p1 = write_zip(
        tmp.path(),
        "bad-overrides.zip",
        &build_zip(|w| add_file(w, "manifest.json", &bad_manifest)),
    );
    assert_eq!(
        inspect_curseforge_pack_with(&p1, PackLimits::default())
            .unwrap_err()
            .code(),
        ErrorCode::ModpackInvalid
    );

    // 2. Đường dẫn traversal bên trong zip overrides
    let ok_manifest = sample_manifest("overrides", json!([]));
    let p2 = write_zip(
        tmp.path(),
        "traversal.zip",
        &build_zip(|w| {
            add_file(w, "manifest.json", &ok_manifest);
            add_file(w, "overrides/../escape.txt", b"evil");
        }),
    );
    assert_eq!(
        inspect_curseforge_pack_with(&p2, PackLimits::default())
            .unwrap_err()
            .code(),
        ErrorCode::ModpackInvalid
    );

    // 3. `manifest.json` là symlink -> từ chối
    let p3 = write_zip(
        tmp.path(),
        "sym-manifest.zip",
        &build_zip(|w| {
            w.add_symlink("manifest.json", "/etc/passwd", SimpleFileOptions::default())
                .unwrap();
        }),
    );
    assert_eq!(
        inspect_curseforge_pack_with(&p3, PackLimits::default())
            .unwrap_err()
            .code(),
        ErrorCode::ModpackInvalid
    );

    // 4. Symlink trong `overrides/` bị bỏ qua (không tạo link trên đĩa)
    let p4 = write_zip(
        tmp.path(),
        "sym-override.zip",
        &build_zip(|w| {
            add_file(w, "manifest.json", &ok_manifest);
            w.add_symlink(
                "overrides/mods/link.jar",
                "../../secret.jar",
                SimpleFileOptions::default(),
            )
            .unwrap();
            add_file(w, "overrides/config/safe.cfg", b"ok");
        }),
    );
    let out4 = tmp.path().join("out4");
    extract_curseforge_overrides_with(&p4, &out4, PackLimits::default()).unwrap();
    assert!(fs::symlink_metadata(out4.join("mods/link.jar")).is_err());
    assert_eq!(
        fs::read_to_string(out4.join("config/safe.cfg")).unwrap(),
        "ok"
    );

    // 5. Zip bomb nói dối header
    let payload = vec![b'Z'; 4096];
    let mut bomb = build_zip(|w| {
        add_file(w, "manifest.json", &ok_manifest);
        add_file(w, "overrides/bomb.bin", &payload);
    });
    patch_zip_uncompressed_size(&mut bomb, 4096, 8);
    let p5 = write_zip(tmp.path(), "bomb.zip", &bomb);
    let limits = PackLimits {
        max_index_bytes: 1024 * 1024,
        max_total_bytes: 512,
        max_files: 10,
    };
    let out5 = tmp.path().join("out5");
    assert_eq!(
        extract_curseforge_overrides_with(&p5, &out5, limits)
            .unwrap_err()
            .code(),
        ErrorCode::ModpackInvalid
    );
}

#[test]
fn server_pack_strips_single_root_dir_and_skips_launch_files() {
    let tmp = tempfile::tempdir().unwrap();
    let nested = build_zip(|w| {
        add_file(w, "MyPack-Server-1.0/mods/example.jar", b"mod-bytes");
        add_file(w, "MyPack-Server-1.0/config/app.toml", b"enabled = true");
        add_file(w, "MyPack-Server-1.0/libraries/net/forge/lib.jar", b"lib");
        add_file(w, "MyPack-Server-1.0/start.sh", b"#!/bin/sh\njava");
        add_file(w, "MyPack-Server-1.0/run.bat", b"@echo off");
        add_file(w, "MyPack-Server-1.0/launch.cmd", b"java");
        add_file(
            w,
            "MyPack-Server-1.0/forge-1.20.1-47.2.0-installer.jar",
            b"installer",
        );
        add_file(w, "MyPack-Server-1.0/forge-1.20.1-47.2.0-shim.jar", b"shim");
        add_file(w, "MyPack-Server-1.0/fabric-server-launch.jar", b"fabric");
    });
    let zip_path = write_zip(tmp.path(), "server-pack.zip", &nested);
    let dest = tmp.path().join("srv-nested");
    let skipped = extract_curseforge_server_pack(&zip_path, &dest).unwrap();

    assert_eq!(
        fs::read(dest.join("mods/example.jar")).unwrap(),
        b"mod-bytes"
    );
    assert_eq!(
        fs::read_to_string(dest.join("config/app.toml")).unwrap(),
        "enabled = true"
    );
    assert!(!dest.join("MyPack-Server-1.0").exists());
    assert!(!dest.join("libraries").exists());
    assert!(!dest.join("start.sh").exists());
    assert!(!dest.join("run.bat").exists());
    assert!(!dest.join("launch.cmd").exists());
    assert!(!dest.join("forge-1.20.1-47.2.0-installer.jar").exists());
    assert!(!dest.join("forge-1.20.1-47.2.0-shim.jar").exists());
    assert!(!dest.join("fabric-server-launch.jar").exists());
    assert_eq!(skipped.len(), 7);

    // Server pack phẳng chỉ chứa `mods/` KHÔNG được bóc mất `mods/`.
    let only_mods = build_zip(|w| add_file(w, "mods/only.jar", b"jar"));
    let dest_flat = tmp.path().join("srv-flat");
    extract_curseforge_server_pack_with(
        &write_zip(tmp.path(), "flat.zip", &only_mods),
        &dest_flat,
        PackLimits::default(),
    )
    .unwrap();
    assert_eq!(fs::read(dest_flat.join("mods/only.jar")).unwrap(), b"jar");
}
