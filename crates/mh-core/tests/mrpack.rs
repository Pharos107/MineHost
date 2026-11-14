//! Kiểm thử phân tích `modrinth.index.json`, kiểm tra đường dẫn và suy ra loader.

#![allow(clippy::unwrap_used)]

use mh_core::{
    LoaderKind, MrpackDependencies, MrpackEnvSupport, MrpackError, MrpackIndex,
    validate_mrpack_path,
};

const REAL_INDEX: &str = include_str!("../../mh-providers/fixtures/mrpack/modrinth.index.json");

fn base_json(deps: &str, path: &str) -> String {
    let encoded_path = serde_json::to_string(path).unwrap();
    format!(
        r#"{{
  "formatVersion": 1,
  "game": "minecraft",
  "versionId": "1.0.0",
  "name": "Test Pack",
  "files": [{{
    "path": {encoded_path},
    "hashes": {{
      "sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "sha512": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    }},
    "downloads": ["https://cdn.modrinth.com/data/test/versions/1/mod.jar"],
    "fileSize": 1234
  }}],
  "dependencies": {deps}
}}"#
    )
}

#[test]
fn parses_real_modrinth_index_fixture() {
    let idx = MrpackIndex::from_json(REAL_INDEX).unwrap();
    assert_eq!(idx.format_version, 1);
    assert_eq!(idx.game, "minecraft");
    assert_eq!(idx.name, "2.4.8");
    assert_eq!(idx.version_id, "2.4.8");
    assert!(idx.summary.is_some());
    assert_eq!(idx.files.len(), 83);

    let rt = idx.resolve_runtime().unwrap();
    assert_eq!(rt.minecraft.as_str(), "26.3");
    assert_eq!(rt.loader, LoaderKind::Fabric);
    assert_eq!(
        rt.loader_version.as_ref().map(|v| v.as_str()),
        Some("0.20.1")
    );

    let client_only = idx.files.iter().filter(|f| f.is_client_only()).count();
    assert_eq!(client_only, 3);
    assert!(
        idx.files
            .iter()
            .filter(|f| f.is_client_only())
            .all(|f| f.env.and_then(|e| e.server) == Some(MrpackEnvSupport::Unsupported))
    );
}

#[test]
fn infers_loader_for_all_supported_types() {
    let cases = [
        (
            r#"{"minecraft":"1.21.4","fabric-loader":"0.16.9"}"#,
            LoaderKind::Fabric,
            Some("0.16.9"),
        ),
        (
            r#"{"minecraft":"1.21.1","quilt-loader":"0.27.0-beta.1"}"#,
            LoaderKind::Quilt,
            Some("0.27.0-beta.1"),
        ),
        (
            r#"{"minecraft":"1.20.1","forge":"47.3.0"}"#,
            LoaderKind::Forge,
            Some("47.3.0"),
        ),
        (
            r#"{"minecraft":"1.21.4","neoforge":"21.4.140"}"#,
            LoaderKind::NeoForge,
            Some("21.4.140"),
        ),
        (r#"{"minecraft":"1.21.4"}"#, LoaderKind::Vanilla, None),
    ];
    for (deps, expected_loader, expected_ver) in cases {
        let json = base_json(deps, "mods/example.jar");
        let idx = MrpackIndex::from_json(&json).unwrap();
        let rt = idx.resolve_runtime().unwrap();
        assert_eq!(rt.loader, expected_loader, "{deps}");
        assert_eq!(
            rt.loader_version.as_ref().map(|v| v.as_str()),
            expected_ver,
            "{deps}"
        );
    }
}

#[test]
fn rejects_bad_format_game_and_dependencies() {
    let mut val: serde_json::Value = serde_json::from_str(REAL_INDEX).unwrap();
    val["formatVersion"] = serde_json::json!(2);
    assert!(matches!(
        MrpackIndex::from_json(&val.to_string()),
        Err(MrpackError::UnsupportedFormat(2))
    ));

    let mut val: serde_json::Value = serde_json::from_str(REAL_INDEX).unwrap();
    val["game"] = serde_json::json!("minetest");
    assert!(matches!(
        MrpackIndex::from_json(&val.to_string()),
        Err(MrpackError::UnsupportedGame(_))
    ));

    assert!(matches!(
        MrpackIndex::from_json("{not json"),
        Err(MrpackError::Json(_))
    ));

    for deps in [
        r#"{"fabric-loader":"0.16.9"}"#,
        r#"{"minecraft":"1.21.4","fabric-loader":"0.16.9","forge":"54.0.0"}"#,
        r#"{"minecraft":"1.21.4","babric-loader":"0.1.0"}"#,
        r#"{"minecraft":"1.21.4","fabric-loader":""}"#,
        r#"{"minecraft":"../bad","fabric-loader":"0.16.9"}"#,
    ] {
        let json = base_json(deps, "mods/a.jar");
        assert!(
            matches!(
                MrpackIndex::from_json(&json),
                Err(MrpackError::InvalidDependencies(_))
            ),
            "phải từ chối {deps}"
        );
    }

    let parsed_deps: MrpackDependencies =
        serde_json::from_str(r#"{"minecraft":"1.21.4","neoforge":"21.4.140"}"#).unwrap();
    assert_eq!(parsed_deps.resolve().unwrap().loader, LoaderKind::NeoForge);
}

#[test]
fn validates_mrpack_relative_paths() {
    for ok in [
        "mods/sodium.jar",
        "config/sub/mod.cfg",
        "options.txt",
        "mods/emojis+26.2-1.0.5.jar",
    ] {
        assert!(validate_mrpack_path(ok).is_ok(), "phải nhận {ok}");
    }
    for bad in [
        "",
        "..",
        "../secret.jar",
        "mods/../../secret.jar",
        "mods/..",
        ".",
        "./mods/a.jar",
        "mods/./a.jar",
        "/etc/passwd",
        r"\Windows\System32\cmd.exe",
        r"mods\a.jar",
        "C:/Windows/a.txt",
        "C:foo.txt",
        "foo.txt:stream",
        "mods//a.jar",
        "mods/a.jar/",
        "mods/\0evil.jar",
    ] {
        assert!(
            matches!(
                validate_mrpack_path(bad),
                Err(MrpackError::InvalidPath { .. })
            ),
            "phải chặn {bad}"
        );
        let json = base_json(r#"{"minecraft":"1.21.4","fabric-loader":"0.16.9"}"#, bad);
        assert!(
            matches!(
                MrpackIndex::from_json(&json),
                Err(MrpackError::InvalidPath { .. })
            ),
            "index phải chặn {bad}"
        );
    }
}
