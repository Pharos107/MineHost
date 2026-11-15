//! Kiểm thử tách bước lập kế hoạch và giải nén overrides cho `.mrpack`.

#![allow(clippy::unwrap_used)]

use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;

use mh_core::ErrorCode;
use mh_providers::{extract_mrpack_overrides, plan_mrpack};
use serde_json::json;
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

const SHA1_ZERO: &str = "0000000000000000000000000000000000000000";
const SHA512_ZERO: &str = "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

fn build_pack(dir: &Path, name: &str) -> std::path::PathBuf {
    let idx = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0.0",
        "name": "Split Pack",
        "files": [{
            "path": "mods/example.jar",
            "hashes": { "sha1": SHA1_ZERO, "sha512": SHA512_ZERO },
            "downloads": ["https://cdn.modrinth.com/data/aaaaaaaa/versions/bbbbbbbb/example.jar"],
            "fileSize": 10
        }],
        "dependencies": { "minecraft": "1.21.4", "fabric-loader": "0.16.9" }
    })
    .to_string();

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (entry, data) in [
        ("modrinth.index.json", idx.as_bytes()),
        ("overrides/mods/example.jar", b"from-overrides" as &[u8]),
        ("overrides/config/a.cfg", b"override"),
        ("server-overrides/config/a.cfg", b"server-override"),
    ] {
        writer.start_file(entry, opts).unwrap();
        writer.write_all(data).unwrap();
    }
    let bytes = writer.finish().unwrap().into_inner();
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn plan_does_not_write_and_extract_overwrites_downloaded_files() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = build_pack(tmp.path(), "split.mrpack");
    let dest = tmp.path().join("server");

    let plan = plan_mrpack(&pack).unwrap();
    assert_eq!(plan.files.len(), 1);
    assert!(!dest.exists(), "lập kế hoạch không được tạo thư mục đích");

    // Giả lập bước tải file trong index trước khi áp overrides.
    fs::create_dir_all(dest.join("mods")).unwrap();
    fs::write(dest.join("mods/example.jar"), b"downloaded-index").unwrap();

    extract_mrpack_overrides(&pack, &dest).unwrap();
    assert_eq!(
        fs::read(dest.join("mods/example.jar")).unwrap(),
        b"from-overrides"
    );
    assert_eq!(
        fs::read_to_string(dest.join("config/a.cfg")).unwrap(),
        "server-override"
    );
}

#[test]
fn plan_rejects_reserved_windows_name_without_touching_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let idx = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0.0",
        "name": "Bad",
        "files": [],
        "dependencies": { "minecraft": "1.21.4" }
    })
    .to_string();
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    writer.start_file("modrinth.index.json", opts).unwrap();
    writer.write_all(idx.as_bytes()).unwrap();
    writer.start_file("overrides/CON.txt", opts).unwrap();
    writer.write_all(b"bad").unwrap();
    let pack = tmp.path().join("bad.mrpack");
    fs::write(&pack, writer.finish().unwrap().into_inner()).unwrap();

    let err = plan_mrpack(&pack).unwrap_err();
    assert_eq!(err.code(), ErrorCode::ModpackInvalid);
}
