//! Kiểm thử đọc `.mrpack`, giải nén `overrides/` + `server-overrides/`, host và zip bomb.

#![allow(clippy::unwrap_used)]

use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;

use mh_core::{ErrorCode, LoaderKind};
use mh_net::{Checksum, HostAllowlist, hosts};
use mh_providers::{PackLimits, read_mrpack, read_mrpack_with};
use serde_json::json;
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

const REAL_INDEX: &str = include_str!("../fixtures/mrpack/modrinth.index.json");
const SHA1_ZERO: &str = "0000000000000000000000000000000000000000";
const SHA512_ZERO: &str = "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

fn sample_file(path: &str, url: &str, server_env: Option<&str>, size: u64) -> serde_json::Value {
    let mut obj = json!({
        "path": path,
        "hashes": { "sha1": SHA1_ZERO, "sha512": SHA512_ZERO },
        "downloads": [url],
        "fileSize": size,
    });
    if let Some(s) = server_env {
        obj["env"] = json!({ "client": "required", "server": s });
    }
    obj
}

fn sample_index(deps: serde_json::Value, files: Vec<serde_json::Value>) -> String {
    json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.2.3",
        "name": "Pack Test",
        "files": files,
        "dependencies": deps,
    })
    .to_string()
}

fn build_zip(build: impl FnOnce(&mut ZipWriter<Cursor<Vec<u8>>>)) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    build(&mut writer);
    writer.finish().unwrap().into_inner()
}

fn write_file_entry(writer: &mut ZipWriter<Cursor<Vec<u8>>>, name: &str, data: &[u8]) {
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer.start_file(name, opts).unwrap();
    writer.write_all(data).unwrap();
}

fn write_zip_file(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn reads_real_fixture_index_and_skips_client_mods() {
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("server");
    let bytes = build_zip(|w| {
        write_file_entry(w, "modrinth.index.json", REAL_INDEX.as_bytes());
        write_file_entry(w, "overrides/config/base.txt", b"base");
    });
    let pack = write_zip_file(tmp.path(), "real.mrpack", &bytes);
    let plan = read_mrpack(&pack, &dest).unwrap();

    assert_eq!(plan.loader, LoaderKind::Fabric);
    assert_eq!(plan.minecraft.as_str(), "26.3");
    assert_eq!(
        plan.loader_version.as_ref().map(|v| v.as_str()),
        Some("0.20.1")
    );
    assert_eq!(plan.files.len(), 80);
    assert_eq!(plan.skipped_client.len(), 3);
    assert!(plan.blocked.is_empty());
    assert!(matches!(plan.files[0].checksum, Checksum::Sha512(_)));
    assert_eq!(plan.files[0].hosts, hosts::MODRINTH);
    assert_eq!(
        fs::read_to_string(dest.join("config/base.txt")).unwrap(),
        "base"
    );
}

#[test]
fn infers_all_four_loaders() {
    let cases = [
        (
            json!({"minecraft": "1.21.4", "fabric-loader": "0.16.9"}),
            LoaderKind::Fabric,
            "0.16.9",
        ),
        (
            json!({"minecraft": "1.21.1", "quilt-loader": "0.26.3"}),
            LoaderKind::Quilt,
            "0.26.3",
        ),
        (
            json!({"minecraft": "1.20.1", "forge": "47.3.0"}),
            LoaderKind::Forge,
            "47.3.0",
        ),
        (
            json!({"minecraft": "1.21.4", "neoforge": "21.4.140"}),
            LoaderKind::NeoForge,
            "21.4.140",
        ),
    ];
    for (deps, loader, ver) in cases {
        let tmp = tempfile::tempdir().unwrap();
        let idx = sample_index(
            deps,
            vec![sample_file(
                "mods/a.jar",
                "https://cdn.modrinth.com/data/a/versions/1/a.jar",
                None,
                10,
            )],
        );
        let bytes = build_zip(|w| write_file_entry(w, "modrinth.index.json", idx.as_bytes()));
        let pack = write_zip_file(tmp.path(), "pack.mrpack", &bytes);
        let plan = read_mrpack(&pack, &tmp.path().join("out")).unwrap();
        assert_eq!(plan.loader, loader);
        assert_eq!(plan.loader_version.unwrap().as_str(), ver);
    }
}

#[test]
fn server_overrides_wins_regardless_of_zip_order_and_ignores_client_or_extra() {
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("server");
    let idx = sample_index(
        json!({"minecraft": "1.21.4", "fabric-loader": "0.16.9"}),
        vec![],
    );
    let bytes = build_zip(|w| {
        write_file_entry(w, "modrinth.index.json", idx.as_bytes());
        // Đặt `server-overrides` đứng trước `overrides` trong zip để kiểm tra thứ tự áp dụng.
        write_file_entry(w, "server-overrides/config/common.cfg", b"server-wins");
        write_file_entry(w, "server-overrides/config/server-only.cfg", b"srv");
        write_file_entry(w, "overrides/config/common.cfg", b"base-loses");
        write_file_entry(w, "overrides/config/base-only.cfg", b"base");
        w.add_directory("overrides/datapacks/", SimpleFileOptions::default())
            .unwrap();
        write_file_entry(w, "client-overrides/config/client.cfg", b"nope");
        write_file_entry(w, "client-overrides/shaderpacks/s.zip", b"nope");
        write_file_entry(w, "README.txt", b"ignored");
        write_file_entry(w, "unknown_dir/extra.txt", b"ignored");
    });
    let pack = write_zip_file(tmp.path(), "layers.mrpack", &bytes);
    read_mrpack(&pack, &dest).unwrap();

    assert_eq!(
        fs::read_to_string(dest.join("config/common.cfg")).unwrap(),
        "server-wins"
    );
    assert_eq!(
        fs::read_to_string(dest.join("config/base-only.cfg")).unwrap(),
        "base"
    );
    assert_eq!(
        fs::read_to_string(dest.join("config/server-only.cfg")).unwrap(),
        "srv"
    );
    assert!(dest.join("datapacks").is_dir());
    assert!(!dest.join("config/client.cfg").exists());
    assert!(!dest.join("shaderpacks").exists());
    assert!(!dest.join("README.txt").exists());
    assert!(!dest.join("unknown_dir").exists());
}

#[test]
fn env_server_unsupported_goes_to_skipped_client() {
    let tmp = tempfile::tempdir().unwrap();
    let idx = sample_index(
        json!({"minecraft": "1.21.4", "fabric-loader": "0.16.9"}),
        vec![
            sample_file(
                "mods/server.jar",
                "https://cdn.modrinth.com/data/a/1/server.jar",
                Some("required"),
                100,
            ),
            sample_file(
                "mods/optional.jar",
                "https://github.com/org/repo/releases/download/v1/opt.jar",
                Some("optional"),
                200,
            ),
            sample_file(
                "mods/no-env.jar",
                "https://gitlab.com/org/repo/-/raw/main/noenv.jar",
                None,
                300,
            ),
            sample_file(
                "mods/client-only.jar",
                "https://cdn.modrinth.com/data/b/1/client.jar",
                Some("unsupported"),
                400,
            ),
        ],
    );
    let bytes = build_zip(|w| write_file_entry(w, "modrinth.index.json", idx.as_bytes()));
    let pack = write_zip_file(tmp.path(), "env.mrpack", &bytes);
    let plan = read_mrpack(&pack, &tmp.path().join("out")).unwrap();

    let keep: Vec<&str> = plan.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        keep,
        ["mods/server.jar", "mods/optional.jar", "mods/no-env.jar"]
    );
    assert_eq!(plan.skipped_client, ["mods/client-only.jar"]);
    assert!(plan.blocked.is_empty());
}

#[test]
fn skips_symlinks_inside_overrides() {
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("server");
    let idx = sample_index(
        json!({"minecraft": "1.21.4", "fabric-loader": "0.16.9"}),
        vec![],
    );
    let bytes = build_zip(|w| {
        write_file_entry(w, "modrinth.index.json", idx.as_bytes());
        w.add_symlink(
            "overrides/mods/link.jar",
            "/etc/passwd",
            SimpleFileOptions::default(),
        )
        .unwrap();
        w.add_symlink(
            "server-overrides/config/link2.cfg",
            "../../outside.cfg",
            SimpleFileOptions::default(),
        )
        .unwrap();
        write_file_entry(w, "overrides/config/safe.cfg", b"ok");
    });
    let pack = write_zip_file(tmp.path(), "symlink.mrpack", &bytes);
    read_mrpack(&pack, &dest).unwrap();

    assert_eq!(
        fs::read_to_string(dest.join("config/safe.cfg")).unwrap(),
        "ok"
    );
    assert!(!dest.join("mods/link.jar").exists());
    assert!(fs::symlink_metadata(dest.join("mods/link.jar")).is_err());
    assert!(fs::symlink_metadata(dest.join("config/link2.cfg")).is_err());
}

#[test]
fn rejects_traversal_and_windows_paths() {
    let valid_idx = sample_index(
        json!({"minecraft": "1.21.4", "fabric-loader": "0.16.9"}),
        vec![],
    );
    let bad_zip_entries = [
        "overrides/../escape.txt",
        "overrides/sub/../../escape.txt",
        "server-overrides/C:/Windows/evil.txt",
        r"overrides\back\slash.txt",
        "overrides/CON.txt",
        "/etc/passwd",
        "../top_escape.txt",
    ];
    for entry_name in bad_zip_entries {
        let tmp = tempfile::tempdir().unwrap();
        let bytes = build_zip(|w| {
            write_file_entry(w, "modrinth.index.json", valid_idx.as_bytes());
            write_file_entry(w, entry_name, b"evil");
        });
        let pack = write_zip_file(tmp.path(), "bad.mrpack", &bytes);
        let err = read_mrpack(&pack, &tmp.path().join("out")).unwrap_err();
        assert_eq!(err.code(), ErrorCode::ModpackInvalid, "{entry_name}: {err}");
    }

    for bad_index_path in ["../evil.jar", "/abs.jar", r"C:\evil.jar", "mods/NUL.jar"] {
        let tmp = tempfile::tempdir().unwrap();
        let idx = sample_index(
            json!({"minecraft": "1.21.4", "fabric-loader": "0.16.9"}),
            vec![sample_file(
                bad_index_path,
                "https://cdn.modrinth.com/data/a/1/a.jar",
                None,
                10,
            )],
        );
        let bytes = build_zip(|w| write_file_entry(w, "modrinth.index.json", idx.as_bytes()));
        let pack = write_zip_file(tmp.path(), "bad_idx.mrpack", &bytes);
        let err = read_mrpack(&pack, &tmp.path().join("out")).unwrap_err();
        assert_eq!(
            err.code(),
            ErrorCode::ModpackInvalid,
            "{bad_index_path}: {err}"
        );
    }
}

#[test]
fn rejects_disallowed_download_host() {
    let tmp = tempfile::tempdir().unwrap();
    let idx = sample_index(
        json!({"minecraft": "1.21.4", "fabric-loader": "0.16.9"}),
        vec![json!({
            "path": "mods/evil.jar",
            "hashes": { "sha1": SHA1_ZERO, "sha512": SHA512_ZERO },
            "downloads": [
                "https://cdn.modrinth.com/data/a/1/ok.jar",
                "https://evil.example.com/payload.jar"
            ],
            "fileSize": 100
        })],
    );
    let bytes = build_zip(|w| write_file_entry(w, "modrinth.index.json", idx.as_bytes()));
    let pack = write_zip_file(tmp.path(), "host.mrpack", &bytes);
    let err = read_mrpack(&pack, &tmp.path().join("out")).unwrap_err();
    assert_eq!(err.code(), ErrorCode::DownloadHostNotAllowed);
    assert_eq!(err.params()["host"], "evil.example.com");

    // Host tùy chỉnh trong test vẫn chạy qua `read_mrpack_with`.
    let custom = HostAllowlist::new(&["cdn.modrinth.com", "evil.example.com"]);
    assert!(read_mrpack_with(&pack, &tmp.path().join("ok"), custom, PackLimits::default()).is_ok());
}

#[test]
fn rejects_missing_or_invalid_index() {
    let tmp = tempfile::tempdir().unwrap();

    let no_index = build_zip(|w| write_file_entry(w, "overrides/a.txt", b"hi"));
    let err = read_mrpack(
        &write_zip_file(tmp.path(), "no.mrpack", &no_index),
        &tmp.path().join("1"),
    )
    .unwrap_err();
    assert_eq!(err.code(), ErrorCode::ModpackInvalid);

    let not_zip = write_zip_file(tmp.path(), "garbage.mrpack", b"not a zip");
    let err = read_mrpack(&not_zip, &tmp.path().join("2")).unwrap_err();
    assert_eq!(err.code(), ErrorCode::ModpackInvalid);

    let bad_json = build_zip(|w| write_file_entry(w, "modrinth.index.json", b"{broken"));
    let err = read_mrpack(
        &write_zip_file(tmp.path(), "bad.mrpack", &bad_json),
        &tmp.path().join("3"),
    )
    .unwrap_err();
    assert_eq!(err.code(), ErrorCode::ModpackInvalid);
}

#[test]
fn blocks_zip_bombs_declared_and_lying_headers_and_file_count() {
    let tmp = tempfile::tempdir().unwrap();
    let small_idx = sample_index(
        json!({"minecraft": "1.21.4", "fabric-loader": "0.16.9"}),
        vec![],
    );

    // 1. Khai báo kích thước vượt ngưỡng trong index.
    let big_declared_idx = sample_index(
        json!({"minecraft": "1.21.4", "fabric-loader": "0.16.9"}),
        vec![sample_file(
            "mods/huge.jar",
            "https://cdn.modrinth.com/data/a/1/huge.jar",
            None,
            2048,
        )],
    );
    let bytes =
        build_zip(|w| write_file_entry(w, "modrinth.index.json", big_declared_idx.as_bytes()));
    let pack = write_zip_file(tmp.path(), "big_idx.mrpack", &bytes);
    let limits = PackLimits {
        max_index_bytes: 1024 * 1024,
        max_total_bytes: 1024,
        max_files: 10,
    };
    let err = read_mrpack_with(&pack, &tmp.path().join("o1"), hosts::MODRINTH, limits).unwrap_err();
    assert_eq!(err.code(), ErrorCode::ModpackInvalid);

    // 2. Header zip nói dối kích thước nhỏ (16 byte) nhưng giải nén thật ra 8192 byte.
    let payload = vec![b'A'; 8192];
    let mut bomb_bytes = build_zip(|w| {
        write_file_entry(w, "modrinth.index.json", small_idx.as_bytes());
        write_file_entry(w, "overrides/bomb.bin", &payload);
    });
    patch_zip_uncompressed_size(&mut bomb_bytes, 8192, 16);
    let bomb_pack = write_zip_file(tmp.path(), "bomb.mrpack", &bomb_bytes);
    let bomb_dest = tmp.path().join("o2");
    let err = read_mrpack_with(&bomb_pack, &bomb_dest, hosts::MODRINTH, limits).unwrap_err();
    assert_eq!(err.code(), ErrorCode::ModpackInvalid);
    assert!(fs::metadata(bomb_dest.join("bomb.bin")).unwrap().len() <= 1024);

    // 3. `modrinth.index.json` vượt quá giới hạn đọc.
    let index_bomb =
        build_zip(|w| write_file_entry(w, "modrinth.index.json", small_idx.as_bytes()));
    let index_pack = write_zip_file(tmp.path(), "idx_limit.mrpack", &index_bomb);
    let tiny_idx_limits = PackLimits {
        max_index_bytes: 32,
        ..limits
    };
    let err = read_mrpack_with(
        &index_pack,
        &tmp.path().join("o3"),
        hosts::MODRINTH,
        tiny_idx_limits,
    )
    .unwrap_err();
    assert_eq!(err.code(), ErrorCode::ModpackInvalid);

    // 4. Tổng số file (index + overrides) vượt quá `max_files`.
    let count_idx = sample_index(
        json!({"minecraft": "1.21.4", "fabric-loader": "0.16.9"}),
        vec![
            sample_file(
                "mods/1.jar",
                "https://cdn.modrinth.com/data/a/1/1.jar",
                None,
                10,
            ),
            sample_file(
                "mods/2.jar",
                "https://cdn.modrinth.com/data/a/1/2.jar",
                None,
                10,
            ),
            sample_file(
                "mods/3.jar",
                "https://cdn.modrinth.com/data/a/1/3.jar",
                None,
                10,
            ),
        ],
    );
    let many_bytes = build_zip(|w| {
        write_file_entry(w, "modrinth.index.json", count_idx.as_bytes());
        write_file_entry(w, "overrides/a.txt", b"1");
        write_file_entry(w, "overrides/b.txt", b"2");
        write_file_entry(w, "overrides/c.txt", b"3");
    });
    let many_pack = write_zip_file(tmp.path(), "many.mrpack", &many_bytes);
    let count_limits = PackLimits {
        max_index_bytes: 1024 * 1024,
        max_total_bytes: 1024 * 1024,
        max_files: 5,
    };
    let err = read_mrpack_with(
        &many_pack,
        &tmp.path().join("o4"),
        hosts::MODRINTH,
        count_limits,
    )
    .unwrap_err();
    assert_eq!(err.code(), ErrorCode::ModpackInvalid);
}

/// Sửa trường `uncompressed_size` trong Local File Header và Central Directory của zip để giả
/// lập archive nói dối kích thước giải nén.
fn patch_zip_uncompressed_size(zip_bytes: &mut [u8], actual: u32, forged: u32) {
    let from = actual.to_le_bytes();
    let to = forged.to_le_bytes();
    let mut patched = 0usize;
    let mut i = 0;
    while i + 4 <= zip_bytes.len() {
        if zip_bytes[i..i + 4] == [0x50, 0x4b, 0x03, 0x04]
            && zip_bytes.get(i + 22..i + 26) == Some(&from)
        {
            zip_bytes[i + 22..i + 26].copy_from_slice(&to);
            patched += 1;
        } else if zip_bytes[i..i + 4] == [0x50, 0x4b, 0x01, 0x02]
            && zip_bytes.get(i + 24..i + 28) == Some(&from)
        {
            zip_bytes[i + 24..i + 28].copy_from_slice(&to);
            patched += 1;
        }
        i += 1;
    }
    assert_eq!(
        patched, 2,
        "phải sửa được cả local header lẫn central directory"
    );
}
