use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use mh_core::{ContentKind, ErrorCode, ModSide};
use sha1::{Digest, Sha1};
use time::macros::datetime;
use zip::write::SimpleFileOptions;

use super::scan::to_hex;
use super::*;

fn make_jar(path: &Path, entries: &[(&str, &[u8])]) -> String {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let file = File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default();
    for (name, data) in entries {
        zip.start_file(*name, opts).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
    let bytes = fs::read(path).unwrap();
    to_hex(&Sha1::digest(&bytes))
}

fn sample_record(
    path: &str,
    sha1: &str,
    source: ContentSource,
    project_id: ProjectId,
) -> ContentRecord {
    ContentRecord {
        path: path.to_owned(),
        sha1: sha1.to_owned(),
        source,
        project_id: Some(project_id),
        version_id: Some("ver1".to_owned()),
        client_only: false,
        from_modpack: false,
        installed_at: datetime!(2026-11-13 10:00 UTC),
        extra: toml::Table::new(),
    }
}

#[test]
fn missing_and_empty_content_toml_are_empty_manifests() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("content.toml");
    assert_eq!(
        ContentManifest::load(&path).unwrap(),
        ContentManifest::default()
    );

    fs::write(&path, "   \n\t\n").unwrap();
    assert_eq!(
        ContentManifest::load(&path).unwrap(),
        ContentManifest::default()
    );
}

#[test]
fn keeps_project_id_types_and_unknown_keys() {
    let raw = r#"
schema = 1
future_root = "keep-me"

[future_table]
flag = true

[[file]]
path = "mods/sodium.jar"
sha1 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
source = "modrinth"
project_id = "AANobbMI"
version_id = "ver1"
client_only = false
from_modpack = true
installed_at = "2026-11-13T10:00:00Z"
extra_file_key = 42

[[file]]
path = "mods/jei.jar"
sha1 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
source = "curseforge"
project_id = 238222
version_id = "4912345"
installed_at = "2026-11-13T11:00:00Z"

[[pending]]
path = "mods/optifine.jar"
project_id = 238222
file_id = 4912345
sha1 = "cccccccccccccccccccccccccccccccccccccccc"
page_url = "https://www.curseforge.com/minecraft/mc-mods/optifine/download/4912345"
extra_pending_key = "yes"
"#;

    let manifest = ContentManifest::from_toml(raw).unwrap();
    assert_eq!(
        manifest.file[0].project_id,
        Some(ProjectId::Str("AANobbMI".into()))
    );
    assert_eq!(manifest.file[1].project_id, Some(ProjectId::Int(238222)));
    assert_eq!(
        manifest.extra.get("future_root"),
        Some(&toml::Value::String("keep-me".into()))
    );
    assert!(manifest.extra.contains_key("future_table"));
    assert_eq!(
        manifest.file[0].extra.get("extra_file_key"),
        Some(&toml::Value::Integer(42))
    );
    assert_eq!(
        manifest.pending[0].extra.get("extra_pending_key"),
        Some(&toml::Value::String("yes".into()))
    );

    let serialized = manifest.to_toml().unwrap();
    assert!(
        serialized.contains("project_id = \"AANobbMI\""),
        "{serialized}"
    );
    assert!(serialized.contains("project_id = 238222"), "{serialized}");

    let reparsed = ContentManifest::from_toml(&serialized).unwrap();
    assert_eq!(reparsed, manifest);
}

#[test]
fn rejects_newer_content_schema() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("content.toml");
    fs::write(&path, "schema = 99\n").unwrap();
    let err = ContentManifest::load(&path).unwrap_err();
    assert_eq!(err.code(), ErrorCode::ConfigInvalid);
}

#[test]
fn scans_mods_plugins_corrupt_and_missing_meta_jars() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let mods = root.join("server/mods");
    fs::create_dir_all(mods.join("nested")).unwrap();

    let fabric_sha1 = make_jar(
        &mods.join("fabric-mod.jar"),
        &[(
            "fabric.mod.json",
            br#"{"id":"sodium","name":"Sodium","version":"0.6.0","environment":"client"}"#,
        )],
    );
    make_jar(
        &mods.join("forge-mod.jar"),
        &[
            (
                "META-INF/mods.toml",
                b"[[mods]]\nmodId=\"jei\"\ndisplayName=\"JEI\"\nversion=\"${file.jarVersion}\"\n",
            ),
            (
                "META-INF/MANIFEST.MF",
                b"Manifest-Version: 1.0\r\nImplementation-Version: 19.21.\r\n 0.247\r\n",
            ),
        ],
    );
    make_jar(&mods.join("no-meta.jar"), &[("assets/hello.txt", b"hi")]);
    fs::write(mods.join("corrupt.jar"), b"not a zip archive").unwrap();
    fs::write(mods.join("notes.txt"), b"ignore me").unwrap();
    // Thư mục con bị bỏ qua (không đệ quy).
    make_jar(
        &mods.join("nested/ignored.jar"),
        &[("fabric.mod.json", br#"{"id":"nested","version":"1.0"}"#)],
    );

    let files = scan_content(root, ContentKind::Mods).unwrap();
    assert_eq!(files.len(), 4);

    let corrupt = files.iter().find(|f| f.file_name == "corrupt.jar").unwrap();
    assert!(matches!(
        corrupt.meta,
        Err(ContentMetaError::InvalidArchive(_))
    ));
    assert_eq!(corrupt.size, b"not a zip archive".len() as u64);
    assert_eq!(corrupt.sha1, to_hex(&Sha1::digest(b"not a zip archive")));
    assert!(corrupt.manual);

    let fabric = files
        .iter()
        .find(|f| f.file_name == "fabric-mod.jar")
        .unwrap();
    assert_eq!(fabric.sha1, fabric_sha1);
    let fmeta = fabric.meta.as_ref().unwrap();
    assert_eq!(fmeta.id, "sodium");
    assert_eq!(fmeta.side, ModSide::Client);
    assert_eq!(fmeta.version.as_deref(), Some("0.6.0"));

    let forge = files
        .iter()
        .find(|f| f.file_name == "forge-mod.jar")
        .unwrap();
    let tmeta = forge.meta.as_ref().unwrap();
    assert_eq!(tmeta.id, "jei");
    assert_eq!(tmeta.version.as_deref(), Some("19.21.0.247"));

    let no_meta = files.iter().find(|f| f.file_name == "no-meta.jar").unwrap();
    assert_eq!(no_meta.meta, Err(ContentMetaError::MissingMetadata));

    // Plugin dùng thư mục `plugins/` và `plugins-disabled/`.
    let plugins = root.join("server/plugins-disabled");
    make_jar(
        &plugins.join("luckperms.jar"),
        &[(
            "plugin.yml",
            b"name: LuckPerms\nversion: 5.4.141\nmain: me.lucko.LP\n",
        )],
    );
    let plugin_files = scan_content(root, ContentKind::Plugins).unwrap();
    assert_eq!(plugin_files.len(), 1);
    assert!(!plugin_files[0].enabled);
    assert_eq!(plugin_files[0].path, "plugins/luckperms.jar");
    assert_eq!(plugin_files[0].meta.as_ref().unwrap().id, "LuckPerms");
}

#[test]
fn respects_metadata_priority_and_size_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let mods = root.join("server/mods");

    make_jar(
        &mods.join("both.jar"),
        &[
            (
                "fabric.mod.json",
                br#"{"id":"from-fabric","version":"1.0"}"#,
            ),
            (
                "quilt.mod.json",
                br#"{"schema_version":1,"quilt_loader":{"id":"from-quilt","version":"2.0"}}"#,
            ),
        ],
    );
    make_jar(
        &mods.join("forge-both.jar"),
        &[
            (
                "META-INF/mods.toml",
                b"[[mods]]\nmodId=\"old-forge\"\nversion=\"1.0\"\n",
            ),
            (
                "META-INF/neoforge.mods.toml",
                b"[[mods]]\nmodId=\"new-neoforge\"\nversion=\"2.0\"\n",
            ),
        ],
    );
    let plugins = root.join("server/plugins");
    make_jar(
        &plugins.join("paper-both.jar"),
        &[
            ("plugin.yml", b"name: BukkitName\nversion: 1.0\nmain: a.B\n"),
            (
                "paper-plugin.yml",
                b"name: PaperName\nversion: 2.0\nmain: a.P\n",
            ),
        ],
    );
    let oversized = vec![b'a'; 1024 * 1024 + 10];
    make_jar(&mods.join("huge.jar"), &[("fabric.mod.json", &oversized)]);

    let files = scan_content(root, ContentKind::Mods).unwrap();
    let both = files.iter().find(|f| f.file_name == "both.jar").unwrap();
    assert_eq!(both.meta.as_ref().unwrap().id, "from-quilt");
    let forge_both = files
        .iter()
        .find(|f| f.file_name == "forge-both.jar")
        .unwrap();
    assert_eq!(forge_both.meta.as_ref().unwrap().id, "new-neoforge");

    let huge = files.iter().find(|f| f.file_name == "huge.jar").unwrap();
    assert_eq!(huge.meta, Err(ContentMetaError::MetadataTooLarge));

    let plugin_files = scan_content(root, ContentKind::Plugins).unwrap();
    assert_eq!(plugin_files[0].meta.as_ref().unwrap().id, "PaperName");
}

#[test]
fn reconciles_renames_deletes_and_toggles_preserving_extra_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let mods = root.join("server/mods");
    let disabled = root.join("server/mods-disabled");
    let content_path = root.join(CONTENT_FILE);

    let sha_a = make_jar(
        &mods.join("alpha.jar"),
        &[("fabric.mod.json", br#"{"id":"alpha","version":"1.0"}"#)],
    );
    let sha_b = make_jar(
        &mods.join("beta.jar"),
        &[("fabric.mod.json", br#"{"id":"beta","version":"1.0"}"#)],
    );
    let sha_c = make_jar(
        &mods.join("gamma.jar"),
        &[("fabric.mod.json", br#"{"id":"gamma","version":"1.0"}"#)],
    );

    let mut rec_a = sample_record(
        "mods/alpha.jar",
        &sha_a,
        ContentSource::Modrinth,
        ProjectId::Str("AA".into()),
    );
    rec_a
        .extra
        .insert("custom_note".into(), toml::Value::String("keep".into()));
    let manifest = ContentManifest {
        schema: 1,
        file: vec![
            rec_a,
            sample_record(
                "mods/beta.jar",
                &sha_b,
                ContentSource::Curseforge,
                ProjectId::Int(12345),
            ),
            sample_record(
                "mods/gamma.jar",
                &sha_c,
                ContentSource::Modpack,
                ProjectId::Str("GG".into()),
            ),
        ],
        pending: vec![PendingContent {
            path: "mods/pending.jar".into(),
            project_id: 99,
            file_id: 88,
            sha1: "deadbeef".into(),
            page_url: "https://example.com".into(),
            extra: toml::Table::new(),
        }],
        extra: toml::Table::from_iter([("root_extra".into(), toml::Value::Boolean(true))]),
    };
    manifest.save(&content_path).unwrap();

    // 1. Đổi tên alpha.jar -> alpha-renamed.jar, xóa gamma.jar, tắt beta.jar.
    fs::rename(mods.join("alpha.jar"), mods.join("alpha-renamed.jar")).unwrap();
    fs::remove_file(mods.join("gamma.jar")).unwrap();
    fs::create_dir_all(&disabled).unwrap();
    fs::rename(mods.join("beta.jar"), disabled.join("beta.jar")).unwrap();

    let files = scan_content(root, ContentKind::Mods).unwrap();
    assert_eq!(files.len(), 2);

    let alpha = files
        .iter()
        .find(|f| f.file_name == "alpha-renamed.jar")
        .unwrap();
    assert!(alpha.enabled);
    assert!(!alpha.manual);
    assert_eq!(alpha.path, "mods/alpha-renamed.jar");
    let rec = alpha.record.as_ref().unwrap();
    assert_eq!(rec.path, "mods/alpha-renamed.jar");
    assert_eq!(
        rec.extra.get("custom_note"),
        Some(&toml::Value::String("keep".into()))
    );

    let beta = files.iter().find(|f| f.file_name == "beta.jar").unwrap();
    assert!(!beta.enabled);
    assert!(!beta.manual);
    assert_eq!(beta.path, "mods/beta.jar");
    assert_eq!(
        beta.record.as_ref().unwrap().project_id,
        Some(ProjectId::Int(12345))
    );

    // Kiểm tra content.toml đã bỏ gamma, cập nhật path của alpha, giữ pending và khóa lạ.
    let updated = ContentManifest::load(&content_path).unwrap();
    assert_eq!(updated.file.len(), 2);
    assert_eq!(updated.pending.len(), 1);
    assert_eq!(
        updated.extra.get("root_extra"),
        Some(&toml::Value::Boolean(true))
    );

    // Bật lại beta.jar và thêm comment vào content.toml: không đổi path nên không ghi đè file.
    fs::rename(disabled.join("beta.jar"), mods.join("beta.jar")).unwrap();
    let with_comment = format!(
        "# giữ nguyên\n{}",
        fs::read_to_string(&content_path).unwrap()
    );
    fs::write(&content_path, &with_comment).unwrap();
    let after_toggle = scan_content(root, ContentKind::Mods).unwrap();
    assert!(
        after_toggle
            .iter()
            .find(|f| f.file_name == "beta.jar")
            .unwrap()
            .enabled
    );
    assert_eq!(fs::read_to_string(&content_path).unwrap(), with_comment);
}

#[cfg(unix)]
#[test]
fn ignores_symlinks_in_mods_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let mods = root.join("server/mods");
    fs::create_dir_all(&mods).unwrap();
    let target = tmp.path().join("outside.jar");
    make_jar(
        &target,
        &[("fabric.mod.json", br#"{"id":"outside","version":"1.0"}"#)],
    );
    std::os::unix::fs::symlink(&target, mods.join("link.jar")).unwrap();

    let files = scan_content(root, ContentKind::Mods).unwrap();
    assert!(files.is_empty());
}

#[test]
fn cache_skips_reopening_unchanged_jars_and_is_thread_safe() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let jar_path = root.join("server/mods/mod.jar");

    make_jar(
        &jar_path,
        &[("fabric.mod.json", br#"{"id":"mod-one","version":"1.0"}"#)],
    );
    let mtime = fs::metadata(&jar_path).unwrap().modified().unwrap();
    let size = fs::metadata(&jar_path).unwrap().len();

    let scanner = ContentScanner::new();
    let first = scanner.scan(root, ContentKind::Mods).unwrap();
    assert_eq!(first[0].meta.as_ref().unwrap().id, "mod-one");
    assert_eq!(scanner.cache_stats(), (0, 1));

    let second = scanner.scan(root, ContentKind::Mods).unwrap();
    assert_eq!(second[0].meta.as_ref().unwrap().id, "mod-one");
    assert_eq!(scanner.cache_stats(), (1, 1));

    // Ghi nội dung mới có cùng độ dài và đặt lại đúng mtime để chứng minh scanner không mở lại file.
    let alt_path = root.join("alt.jar");
    make_jar(
        &alt_path,
        &[("fabric.mod.json", br#"{"id":"mod-two","version":"2.0"}"#)],
    );
    assert_eq!(fs::metadata(&alt_path).unwrap().len(), size);
    fs::copy(&alt_path, &jar_path).unwrap();
    File::options()
        .write(true)
        .open(&jar_path)
        .unwrap()
        .set_modified(mtime)
        .unwrap();

    let cached_hit = scanner.scan(root, ContentKind::Mods).unwrap();
    assert_eq!(cached_hit[0].meta.as_ref().unwrap().id, "mod-one");
    assert_eq!(scanner.cache_stats(), (2, 1));

    // Đổi size để làm mất hiệu lực cache -> đọc được mod-two.
    make_jar(
        &jar_path,
        &[("fabric.mod.json", br#"{"id":"mod-two","version":"2.0.0"}"#)],
    );
    let invalidated = scanner.scan(root, ContentKind::Mods).unwrap();
    assert_eq!(invalidated[0].meta.as_ref().unwrap().id, "mod-two");
    assert_eq!(scanner.cache_stats(), (2, 2));

    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                let res = scanner.scan(root, ContentKind::Mods).unwrap();
                assert_eq!(res[0].meta.as_ref().unwrap().id, "mod-two");
            });
        }
    });
}
