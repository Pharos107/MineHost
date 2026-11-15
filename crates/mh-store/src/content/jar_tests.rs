#![allow(clippy::unwrap_used)]

use std::io::{Cursor, Write};

use mh_core::ModMetaFormat;
use zip::write::SimpleFileOptions;

use super::{MAX_NESTED_DEPTH, MAX_NESTED_OPENED, read_jar_metadata};

fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn fabric_jar(id: &str, extra_json: &str, entries: &[(&str, &[u8])]) -> Vec<u8> {
    let meta = format!(r#"{{"schemaVersion":1,"id":"{id}","version":"1.0.0"{extra_json}}}"#);
    let mut all: Vec<(&str, &[u8])> = vec![("fabric.mod.json", meta.as_bytes())];
    all.extend_from_slice(entries);
    zip_bytes(&all)
}

fn ids(jar: &[u8]) -> Vec<String> {
    let meta = read_jar_metadata(Cursor::new(jar.to_vec()));
    let mut ids: Vec<String> = meta.bundled.iter().map(|m| m.id.clone()).collect();
    ids.sort();
    ids
}

#[test]
fn reads_fabric_nested_jars_recursively() {
    let leaf = fabric_jar("leaf", "", &[]);
    let mid = fabric_jar(
        "mid",
        r#","provides":["mid-alias"],"jars":[{"file":"META-INF/jars/leaf.jar"}]"#,
        &[("META-INF/jars/leaf.jar", &leaf)],
    );
    let outer = fabric_jar(
        "outer",
        r#","jars":[{"file":"META-INF/jars/mid.jar"},{"file":"META-INF/jars/absent.jar"}]"#,
        &[("META-INF/jars/mid.jar", &mid)],
    );

    let meta = read_jar_metadata(Cursor::new(outer));
    assert_eq!(meta.meta.as_ref().unwrap().id, "outer");
    let bundled: Vec<(&str, &[String])> = meta
        .bundled
        .iter()
        .map(|m| (m.id.as_str(), m.provides.as_slice()))
        .collect();
    assert_eq!(
        bundled,
        vec![("mid", &["mid-alias".to_owned()][..]), ("leaf", &[][..])]
    );
}

#[test]
fn reads_forge_jarjar_metadata() {
    let inner = zip_bytes(&[(
        "META-INF/mods.toml",
        b"[[mods]]\nmodId=\"inner\"\nversion=\"2.0\"\n",
    )]);
    let jarjar = br#"{"jars":[{"path":"META-INF/jarjar/inner.jar"}]}"#;
    let outer = zip_bytes(&[
        ("META-INF/mods.toml", b"[[mods]]\nmodId=\"outer\"\n"),
        ("META-INF/jarjar/metadata.json", jarjar),
        ("META-INF/jarjar/inner.jar", &inner),
    ]);
    assert_eq!(ids(&outer), vec!["inner".to_owned()]);
}

#[test]
fn keeps_one_meta_per_format_for_multi_loader_jars() {
    let jar = zip_bytes(&[
        (
            "quilt.mod.json",
            br#"{"schema_version":1,"quilt_loader":{"id":"dual","version":"1.0","depends":["quilt_only"]}}"#,
        ),
        ("fabric.mod.json", br#"{"id":"dual","version":"1.0"}"#),
    ]);
    let meta = read_jar_metadata(Cursor::new(jar));
    let formats: Vec<ModMetaFormat> = meta.format_metas.iter().map(|(f, _)| *f).collect();
    assert_eq!(
        formats,
        vec![ModMetaFormat::QuiltModJson, ModMetaFormat::FabricModJson]
    );
    assert_eq!(
        meta.meta.unwrap().depends.len(),
        1,
        "bản chính vẫn là Quilt"
    );
    assert!(meta.format_metas[1].1.depends.is_empty());
}

#[test]
fn stops_at_max_depth_and_skips_broken_nested_jars() {
    let mut jar = fabric_jar("level0", "", &[]);
    for level in 1..=MAX_NESTED_DEPTH + 2 {
        jar = fabric_jar(
            &format!("level{level}"),
            r#","jars":[{"file":"inner.jar"},{"file":"broken.jar"}]"#,
            &[("inner.jar", &jar), ("broken.jar", b"not a zip")],
        );
    }
    // Jar ngoài cùng là level5; chỉ đọc 3 tầng lồng: level4, level3, level2.
    assert_eq!(
        ids(&jar),
        vec![
            "level2".to_owned(),
            "level3".to_owned(),
            "level4".to_owned()
        ]
    );
}

#[test]
fn caps_nested_jars_opened_even_without_metadata() {
    let names: Vec<String> = (0..MAX_NESTED_OPENED + 50)
        .map(|i| format!("lib{i:04}.jar"))
        .collect();
    let jars_json: Vec<String> = names
        .iter()
        .map(|n| format!(r#"{{"file":"{n}"}}"#))
        .collect();
    let extra = format!(r#","jars":[{}]"#, jars_json.join(","));
    let plain = zip_bytes(&[("a.class", b"x")]);
    let entries: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), &plain[..])).collect();
    // Thêm một mod thật ở cuối danh sách đã sắp: vượt ngân sách thì không được đọc tới.
    let last = fabric_jar("late-mod", "", &[]);
    let mut entries = entries;
    let extra = extra.replacen("]", r#",{"file":"zz-late.jar"}]"#, 1);
    entries.push(("zz-late.jar", &last));
    let outer = fabric_jar("outer", &extra, &entries);
    assert!(ids(&outer).is_empty());
}
