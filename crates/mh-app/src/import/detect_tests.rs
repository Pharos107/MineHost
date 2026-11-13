use std::io::Write;
use std::path::Path;

use mh_core::{LoaderKind, McVersion};
use zip::write::SimpleFileOptions;

use super::{has_ignored_jvm_args, inspect, parse_history};

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
    assert!(!found.ignored_jvm_args);
    assert_eq!(
        (found.loader, found.minecraft, found.jar, found.launch),
        (None, None, None, None)
    );
}

#[test]
fn detects_fabric_quilt_forge_and_neoforge() {
    // Fabric qua MANIFEST.MF + tên jar
    let fabric = tempfile::tempdir().unwrap();
    write_jar(
        &fabric
            .path()
            .join("fabric-server-mc.1.21.4-loader.0.16.10-launcher.1.0.1.jar"),
        &[(
            "META-INF/MANIFEST.MF",
            "Manifest-Version: 1.0\nMain-Class: net.fabricmc.installer.ServerLauncher\n",
        )],
    );
    let found = inspect(fabric.path());
    assert_eq!(found.loader, Some(LoaderKind::Fabric));
    assert_eq!(found.minecraft.as_ref().map(|v| v.as_str()), Some("1.21.4"));
    assert_eq!(
        found.loader_version.as_ref().map(|v| v.as_str()),
        Some("0.16.10")
    );
    assert!(matches!(found.launch, Some(mh_core::Launch::Jar(_))));

    // Quilt qua MANIFEST.MF + libraries + versions
    let quilt = tempfile::tempdir().unwrap();
    write_jar(
        &quilt.path().join("quilt-server-launch.jar"),
        &[(
            "META-INF/MANIFEST.MF",
            "Manifest-Version: 1.0\nMain-Class: org.quiltmc.loader.impl.launch.server.QuiltServerLauncher\n",
        )],
    );
    std::fs::create_dir_all(
        quilt
            .path()
            .join("libraries/org/quiltmc/quilt-loader/0.27.1"),
    )
    .unwrap();
    std::fs::create_dir_all(quilt.path().join("versions/1.21.4")).unwrap();
    let found = inspect(quilt.path());
    assert_eq!(found.loader, Some(LoaderKind::Quilt));
    assert_eq!(found.minecraft.as_ref().map(|v| v.as_str()), Some("1.21.4"));
    assert_eq!(
        found.loader_version.as_ref().map(|v| v.as_str()),
        Some("0.27.1")
    );

    // Forge qua args_file
    let forge = tempfile::tempdir().unwrap();
    let forge_dir = forge
        .path()
        .join("libraries/net/minecraftforge/forge/1.21.4-54.1.0");
    std::fs::create_dir_all(&forge_dir).unwrap();
    std::fs::write(forge_dir.join("win_args.txt"), "-p ...").unwrap();
    let found = inspect(forge.path());
    assert_eq!(found.loader, Some(LoaderKind::Forge));
    assert_eq!(found.minecraft.as_ref().map(|v| v.as_str()), Some("1.21.4"));
    assert_eq!(
        found.loader_version.as_ref().map(|v| v.as_str()),
        Some("54.1.0")
    );
    assert_eq!(
        found.launch,
        Some(mh_core::Launch::ArgsFile(
            mh_core::ArgsFilePath::parse(
                "libraries/net/minecraftforge/forge/1.21.4-54.1.0/{os}_args.txt"
            )
            .unwrap()
        ))
    );

    // NeoForge qua args_file
    let neo = tempfile::tempdir().unwrap();
    let neo_dir = neo.path().join("libraries/net/neoforged/neoforge/21.4.140");
    std::fs::create_dir_all(&neo_dir).unwrap();
    std::fs::write(neo_dir.join("unix_args.txt"), "-p ...").unwrap();
    let found = inspect(neo.path());
    assert_eq!(found.loader, Some(LoaderKind::NeoForge));
    assert_eq!(found.minecraft.as_ref().map(|v| v.as_str()), Some("1.21.4"));
    assert_eq!(
        found.loader_version.as_ref().map(|v| v.as_str()),
        Some("21.4.140")
    );
    assert!(matches!(found.launch, Some(mh_core::Launch::ArgsFile(_))));
}

#[test]
fn user_jvm_args_ignores_comments_and_blank_lines() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("user_jvm_args.txt"),
        "# -Xmx4G\n\n   # chú thích\n",
    )
    .unwrap();
    assert!(!has_ignored_jvm_args(dir.path()));

    std::fs::write(
        dir.path().join("user_jvm_args.txt"),
        "# chú thích\n-XX:+UseG1GC\n",
    )
    .unwrap();
    assert!(has_ignored_jvm_args(dir.path()));
}
