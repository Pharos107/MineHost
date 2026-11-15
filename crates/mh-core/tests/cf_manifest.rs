//! Kiểm thử phân tích `manifest.json` CurseForge, kiểm `overrides` và suy ra loader.

#![allow(clippy::unwrap_used)]

use mh_core::{CfManifest, CfManifestError, LoaderKind, validate_cf_overrides};

fn base_json(mc: &str, loaders: &str, overrides: &str) -> String {
    let encoded_overrides = serde_json::to_string(overrides).unwrap();
    format!(
        r#"{{
  "minecraft": {{
    "version": "{mc}",
    "modLoaders": {loaders}
  }},
  "manifestType": "minecraftModpack",
  "manifestVersion": 1,
  "name": "Test CurseForge Pack",
  "version": "1.2.3",
  "author": "Tester",
  "files": [
    {{ "projectID": 238222, "fileID": 4912345, "required": true }},
    {{ "projectID": 306612, "fileID": 5200001 }}
  ],
  "overrides": {encoded_overrides}
}}"#
    )
}

#[test]
fn resolves_all_four_loaders_and_vanilla() {
    let cases = [
        (
            "1.20.1",
            r#"[{"id":"forge-47.2.0","primary":true}]"#,
            LoaderKind::Forge,
            Some("47.2.0"),
        ),
        (
            "1.20.1",
            r#"[{"id":"forge-1.20.1-47.2.0","primary":true}]"#,
            LoaderKind::Forge,
            Some("47.2.0"),
        ),
        (
            "1.21.1",
            r#"[{"id":"neoforge-21.1.80","primary":true}]"#,
            LoaderKind::NeoForge,
            Some("21.1.80"),
        ),
        (
            "1.21.1",
            r#"[{"id":"fabric-0.16.9","primary":true}]"#,
            LoaderKind::Fabric,
            Some("0.16.9"),
        ),
        (
            "1.21.1",
            r#"[{"id":"quilt-0.27.0-beta.1","primary":true}]"#,
            LoaderKind::Quilt,
            Some("0.27.0-beta.1"),
        ),
        ("1.21.4", "[]", LoaderKind::Vanilla, None),
    ];

    for (mc, loaders, expected_loader, expected_ver) in cases {
        let json = base_json(mc, loaders, "overrides");
        let manifest = CfManifest::from_json(&json).unwrap();
        let rt = manifest.resolve_runtime().unwrap();
        assert_eq!(rt.minecraft.as_str(), mc);
        assert_eq!(rt.loader, expected_loader);
        assert_eq!(rt.loader_version.as_ref().map(|v| v.as_str()), expected_ver);
        assert_eq!(manifest.files.len(), 2);
        assert!(manifest.files[0].required);
        assert!(manifest.files[1].required);
    }
}

#[test]
fn picks_primary_among_multiple_loaders() {
    let json = base_json(
        "1.21.1",
        r#"[{"id":"fabric-0.15.0","primary":false},{"id":"fabric-0.16.9","primary":true}]"#,
        "overrides",
    );
    let rt = CfManifest::from_json(&json)
        .unwrap()
        .resolve_runtime()
        .unwrap();
    assert_eq!(rt.loader, LoaderKind::Fabric);
    assert_eq!(rt.loader_version.unwrap().as_str(), "0.16.9");

    // Một loader duy nhất dù `primary: false` vẫn chọn được.
    let single = base_json(
        "1.21.1",
        r#"[{"id":"quilt-0.26.0","primary":false}]"#,
        "overrides",
    );
    let rt = CfManifest::from_json(&single)
        .unwrap()
        .resolve_runtime()
        .unwrap();
    assert_eq!(rt.loader, LoaderKind::Quilt);
}

#[test]
fn rejects_invalid_manifest_type_version_and_loaders() {
    let bad_type = r#"{
        "minecraft":{"version":"1.21.1","modLoaders":[]},
        "manifestType":"otherPack","manifestVersion":1,"name":"x","version":"1","files":[],"overrides":"overrides"
    }"#;
    assert!(matches!(
        CfManifest::from_json(bad_type),
        Err(CfManifestError::UnsupportedType(_))
    ));

    let bad_ver = r#"{
        "minecraft":{"version":"1.21.1","modLoaders":[]},
        "manifestType":"minecraftModpack","manifestVersion":2,"name":"x","version":"1","files":[],"overrides":"overrides"
    }"#;
    assert!(matches!(
        CfManifest::from_json(bad_ver),
        Err(CfManifestError::UnsupportedVersion(2))
    ));

    for loaders in [
        r#"[{"id":"babric-1.0","primary":true}]"#,
        r#"[{"id":"forge-","primary":true}]"#,
        r#"[{"id":"forge-47.2.0","primary":true},{"id":"fabric-0.16.9","primary":true}]"#,
        r#"[{"id":"forge-47.2.0","primary":false},{"id":"fabric-0.16.9","primary":false}]"#,
    ] {
        let json = base_json("1.21.1", loaders, "overrides");
        assert!(
            matches!(
                CfManifest::from_json(&json),
                Err(CfManifestError::InvalidRuntime(_))
            ),
            "phải từ chối {loaders}"
        );
    }
}

#[test]
fn rejects_unsafe_overrides_directory_names() {
    assert!(validate_cf_overrides("overrides").is_ok());
    assert!(validate_cf_overrides("server-overrides").is_ok());

    for bad in [
        "",
        ".",
        "..",
        "../evil",
        "overrides/sub",
        "overrides/",
        "/etc",
        r"\windows",
        "C:evil",
        "bad:stream",
        "bad\0dir",
    ] {
        assert!(
            matches!(
                validate_cf_overrides(bad),
                Err(CfManifestError::InvalidOverrides { .. })
            ),
            "phải chặn overrides `{bad}`"
        );
        let json = base_json("1.21.1", r#"[{"id":"fabric-0.16.9","primary":true}]"#, bad);
        assert!(
            matches!(
                CfManifest::from_json(&json),
                Err(CfManifestError::InvalidOverrides { .. })
            ),
            "manifest phải chặn overrides `{bad}`"
        );
    }
}
