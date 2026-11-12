use std::io::Write;
use std::path::Path;

use mh_core::{LoaderKind, McVersion};
use zip::write::SimpleFileOptions;

use super::{inspect, parse_history};

fn v(id: &str) -> Option<McVersion> {
    Some(McVersion::parse(id).unwrap())
}

/// Jar giả: chỉ có các mục cần để nhận diện, nội dung rỗng.
fn write_jar(path: &Path, entries: &[(&str, &str)]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, body) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn history_strings_from_paper_and_purpur() {
    assert_eq!(
        parse_history("git-Paper-232 (MC: 1.21.4)"),
        (Some(LoaderKind::Paper), v("1.21.4"))
    );
    assert_eq!(
        parse_history("git-Purpur-2416 (MC: 1.21.4)"),
        (Some(LoaderKind::Purpur), v("1.21.4"))
    );
    assert_eq!(
        parse_history("1.21.4-232-e8d6a8c (MC: 1.21.4)"),
        (None, v("1.21.4"))
    );
    assert_eq!(parse_history("không rõ"), (None, None));
}

#[test]
fn paper_folder_is_read_from_version_history() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("version_history.json"),
        r#"{"currentVersion":"git-Paper-232 (MC: 1.21.4)"}"#,
    )
    .unwrap();
    // Jar không phải zip: vẫn dùng được làm jar chạy, loại lấy từ version_history.
    std::fs::write(dir.path().join("paper-1.21.4-232.jar"), b"not a zip").unwrap();
    let found = inspect(dir.path());
    assert_eq!(found.loader, Some(LoaderKind::Paper));
    assert_eq!(found.minecraft, v("1.21.4"));
    assert_eq!(found.jar.unwrap().as_str(), "paper-1.21.4-232.jar");
    assert!(!found.has_mods);
}

#[test]
fn vanilla_and_purpur_jars_are_recognized() {
    let dir = tempfile::tempdir().unwrap();
    write_jar(
        &dir.path().join("server.jar"),
        &[
            ("net/minecraft/bundler/Main.class", ""),
            ("version.json", r#"{"id":"1.21.4"}"#),
        ],
    );
    let found = inspect(dir.path());
    assert_eq!(
        (found.loader, found.minecraft),
        (Some(LoaderKind::Vanilla), v("1.21.4"))
    );
    assert_eq!(found.jar.unwrap().as_str(), "server.jar");

    let dir = tempfile::tempdir().unwrap();
    write_jar(
        &dir.path().join("purpur-1.20.4-2176.jar"),
        &[
            ("io/papermc/paperclip/Main.class", ""),
            ("version.json", r#"{"id":"1.20.4"}"#),
        ],
    );
    let found = inspect(dir.path());
    assert_eq!(
        (found.loader, found.minecraft),
        (Some(LoaderKind::Purpur), v("1.20.4"))
    );
}

#[test]
fn mods_folder_and_empty_folder() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("mods")).unwrap();
    let found = inspect(dir.path());
    assert!(found.has_mods);
    assert_eq!(
        (found.loader, found.minecraft, found.jar),
        (None, None, None)
    );
}
