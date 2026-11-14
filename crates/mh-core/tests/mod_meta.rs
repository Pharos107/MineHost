//! Kiểm thử tích hợp parser metadata trên 20 fixture mod thật từ Modrinth.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use mh_core::{
    ModDependency, ModMeta, ModMetaError, ModMetaFormat, ModSide, matches_version_req,
    parse_mod_meta,
};

struct ExpectedFixture {
    dir: &'static str,
    file: &'static str,
    format: ModMetaFormat,
    id: &'static str,
    name: &'static str,
    version: Option<&'static str>,
    side: ModSide,
    key_deps: &'static [(&'static str, &'static str, bool)],
    absent_deps: &'static [&'static str],
}

const FIXTURES: &[ExpectedFixture] = &[
    ExpectedFixture {
        dir: "sodium",
        file: "fabric.mod.json",
        format: ModMetaFormat::FabricModJson,
        id: "sodium",
        name: "Sodium",
        version: Some("0.8.13+mc1.21.1"),
        side: ModSide::Client,
        key_deps: &[("fabric-rendering-fluids-v1", ">=2.0.0", true)],
        absent_deps: &["minecraft", "fabricloader"],
    },
    ExpectedFixture {
        dir: "lithium",
        file: "fabric.mod.json",
        format: ModMetaFormat::FabricModJson,
        id: "lithium",
        name: "Lithium",
        version: Some("0.15.4+mc1.21.1"),
        side: ModSide::Both,
        key_deps: &[],
        absent_deps: &["minecraft", "fabricloader"],
    },
    ExpectedFixture {
        dir: "iris",
        file: "fabric.mod.json",
        format: ModMetaFormat::FabricModJson,
        id: "iris",
        name: "Iris",
        version: Some("1.8.14-beta.1+mc1.21.1"),
        side: ModSide::Client,
        key_deps: &[("sodium", "0.8.x", true)],
        absent_deps: &["minecraft", "fabricloader"],
    },
    ExpectedFixture {
        dir: "modmenu",
        file: "fabric.mod.json",
        format: ModMetaFormat::FabricModJson,
        id: "modmenu",
        name: "Mod Menu",
        version: Some("11.0.5"),
        side: ModSide::Client,
        key_deps: &[
            ("fabric-screen-api-v1", ">=1.0.4", true),
            ("fabric-resource-loader-v0", "*", true),
        ],
        absent_deps: &["minecraft", "fabricloader"],
    },
    ExpectedFixture {
        dir: "fabric-api",
        file: "fabric.mod.json",
        format: ModMetaFormat::FabricModJson,
        id: "fabric-api",
        name: "Fabric API",
        version: Some("0.116.18+1.21.1"),
        side: ModSide::Both,
        key_deps: &[],
        absent_deps: &["minecraft", "fabricloader", "java"],
    },
    ExpectedFixture {
        dir: "ferrite-core",
        file: "fabric.mod.json",
        format: ModMetaFormat::FabricModJson,
        id: "ferritecore",
        name: "FerriteCore",
        version: Some("7.0.3"),
        side: ModSide::Both,
        key_deps: &[],
        absent_deps: &["minecraft", "fabricloader"],
    },
    ExpectedFixture {
        dir: "entityculling",
        file: "fabric.mod.json",
        format: ModMetaFormat::FabricModJson,
        id: "entityculling",
        name: "EntityCulling",
        version: Some("1.12.1"),
        side: ModSide::Both,
        key_deps: &[("fabric-api", "*", true)],
        absent_deps: &["minecraft"],
    },
    ExpectedFixture {
        dir: "qsl",
        file: "quilt.mod.json",
        format: ModMetaFormat::QuiltModJson,
        id: "quilted_fabric_api",
        name: "Quilted Fabric API",
        version: Some("7.7.0+0.92.2-1.20.1"),
        side: ModSide::Both,
        key_deps: &[
            ("quilt_base", "*", true),
            ("quilted_fabric_command_api_v1", "*", false),
        ],
        absent_deps: &["minecraft", "quilt_loader", "java"],
    },
    ExpectedFixture {
        dir: "ok-zoomer",
        file: "quilt.mod.json",
        format: ModMetaFormat::QuiltModJson,
        id: "ok_zoomer",
        name: "Ok Zoomer",
        version: Some("5.0.0-beta.9+1.19"),
        side: ModSide::Client,
        key_deps: &[
            ("libzoomer", ">=0.4.0", true),
            ("quilted_fabric_api", ">=1.0.0-", true),
        ],
        absent_deps: &["quilt_loader", "java"],
    },
    ExpectedFixture {
        dir: "effective",
        file: "quilt.mod.json",
        format: ModMetaFormat::QuiltModJson,
        id: "effective",
        name: "Effective",
        version: Some("2.1.1+1.19.2"),
        side: ModSide::Both,
        key_deps: &[
            ("quilted_fabric_api", ">=4.0.0-", true),
            ("lodestone", "*", true),
        ],
        absent_deps: &["minecraft", "quilt_loader"],
    },
    ExpectedFixture {
        dir: "oculus",
        file: "mods.toml",
        format: ModMetaFormat::ModsToml,
        id: "oculus",
        name: "Oculus",
        version: Some("1.8.0"),
        side: ModSide::Both,
        key_deps: &[("embeddium", "[0.3.1,)", false)],
        absent_deps: &["minecraft"],
    },
    ExpectedFixture {
        dir: "create",
        file: "mods.toml",
        format: ModMetaFormat::ModsToml,
        id: "create",
        name: "Create",
        version: Some("6.0.8"),
        side: ModSide::Both,
        key_deps: &[
            ("flywheel", "[1.0.0,2.0)", true),
            ("ponder", "[0.8,)", true),
            ("jei", "[15.19.0,)", false),
        ],
        absent_deps: &["minecraft", "forge"],
    },
    ExpectedFixture {
        dir: "farmers-delight",
        file: "mods.toml",
        format: ModMetaFormat::ModsToml,
        id: "farmersdelight",
        name: "Farmer's Delight",
        version: Some("1.20.1-1.3.4"),
        side: ModSide::Both,
        key_deps: &[("crafttweaker", "[14.0.6,)", false)],
        absent_deps: &["minecraft", "forge"],
    },
    ExpectedFixture {
        dir: "embeddium",
        file: "mods.toml",
        format: ModMetaFormat::ModsToml,
        id: "embeddium",
        name: "Embeddium",
        version: Some("0.3.31+mc1.20.1"),
        side: ModSide::Both,
        key_deps: &[
            ("oculus", "(1.6.15,)", false),
            (
                "textrues_embeddium_options",
                "[0.0.0-NOT-COMPATIBLE]",
                false,
            ),
        ],
        absent_deps: &[],
    },
    ExpectedFixture {
        dir: "geckolib",
        file: "mods.toml",
        format: ModMetaFormat::ModsToml,
        id: "geckolib",
        name: "GeckoLib 4",
        version: Some("4.8.4"),
        side: ModSide::Both,
        key_deps: &[("geckoanimfix", "[9.removeoculusgeckolibcompat,)", false)],
        absent_deps: &["minecraft", "forge"],
    },
    ExpectedFixture {
        dir: "jei",
        file: "neoforge.mods.toml",
        format: ModMetaFormat::NeoForgeModsToml,
        id: "jei",
        name: "Just Enough Items",
        version: Some("19.70.0.469"),
        side: ModSide::Both,
        key_deps: &[
            ("mezz_config", "[0.6.10,1.0.0)", true),
            ("mezz_config_gui", "[0.5.5,)", false),
        ],
        absent_deps: &["minecraft", "neoforge"],
    },
    ExpectedFixture {
        dir: "jade",
        file: "neoforge.mods.toml",
        format: ModMetaFormat::NeoForgeModsToml,
        id: "jade",
        name: "Jade",
        version: None,
        side: ModSide::Both,
        key_deps: &[],
        absent_deps: &["neoforge"],
    },
    ExpectedFixture {
        dir: "waystones",
        file: "neoforge.mods.toml",
        format: ModMetaFormat::NeoForgeModsToml,
        id: "waystones",
        name: "Waystones",
        version: Some("21.1.46"),
        side: ModSide::Both,
        key_deps: &[("balm", "[21.0.39,)", true)],
        absent_deps: &["minecraft", "neoforge"],
    },
    ExpectedFixture {
        dir: "curios",
        file: "neoforge.mods.toml",
        format: ModMetaFormat::NeoForgeModsToml,
        id: "curios",
        name: "Curios API",
        version: Some("9.5.1+1.21.1"),
        side: ModSide::Both,
        key_deps: &[],
        absent_deps: &["minecraft", "neoforge"],
    },
    ExpectedFixture {
        dir: "appleskin",
        file: "neoforge.mods.toml",
        format: ModMetaFormat::NeoForgeModsToml,
        id: "appleskin",
        name: "AppleSkin",
        version: None,
        side: ModSide::Client,
        key_deps: &[],
        absent_deps: &["neoforge"],
    },
];

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/meta")
}

fn assert_fixture(meta: &ModMeta, exp: &ExpectedFixture) {
    assert_eq!(meta.id, exp.id, "{}", exp.dir);
    assert_eq!(meta.name, exp.name, "{}", exp.dir);
    assert_eq!(meta.version.as_deref(), exp.version, "{}", exp.dir);
    assert_eq!(meta.side, exp.side, "{}", exp.dir);
    for &(dep_id, req, required) in exp.key_deps {
        let want = ModDependency {
            id: dep_id.to_owned(),
            version_req: req.to_owned(),
            required,
        };
        assert!(meta.depends.contains(&want), "{} thiếu {want:?}", exp.dir);
    }
    for &absent in exp.absent_deps {
        assert!(
            !meta
                .depends
                .iter()
                .any(|d| d.id.eq_ignore_ascii_case(absent)),
            "{} chứa `{absent}`",
            exp.dir
        );
    }
}

#[test]
fn parses_all_real_mod_fixtures() {
    let root = fixture_root();
    let mut seen_dirs = BTreeSet::new();
    let mut client_only_count = 0usize;

    for exp in FIXTURES {
        let path = root.join(exp.dir).join(exp.file);
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        let meta = parse_mod_meta(exp.format, &text)
            .unwrap_or_else(|e| panic!("{} parse thất bại: {e}", exp.dir));
        assert_fixture(&meta, exp);
        if meta.side.is_client_only() {
            client_only_count += 1;
        }
        assert!(seen_dirs.insert(exp.dir));
    }

    assert_eq!(FIXTURES.len(), 20);
    assert!(client_only_count >= 3, "client_only = {client_only_count}");

    let actual_dirs: BTreeSet<String> = fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    let expected_dirs: BTreeSet<String> = FIXTURES.iter().map(|f| f.dir.to_owned()).collect();
    assert_eq!(actual_dirs, expected_dirs);
}

#[test]
fn iris_sodium_wildcard_matches_sodium_fixture() {
    let iris_text = fs::read_to_string(fixture_root().join("iris/fabric.mod.json")).unwrap();
    let iris = parse_mod_meta(ModMetaFormat::FabricModJson, &iris_text).unwrap();
    let sodium_dep = iris.depends.iter().find(|d| d.id == "sodium").unwrap();
    assert_eq!(
        matches_version_req("0.8.13+mc1.21.1", &sodium_dep.version_req),
        Some(true)
    );
}

#[test]
fn recognizes_archive_paths() {
    assert_eq!(
        ModMetaFormat::from_archive_path("fabric.mod.json"),
        Some(ModMetaFormat::FabricModJson)
    );
    assert_eq!(
        ModMetaFormat::from_archive_path("quilt.mod.json"),
        Some(ModMetaFormat::QuiltModJson)
    );
    assert_eq!(
        ModMetaFormat::from_archive_path("META-INF/mods.toml"),
        Some(ModMetaFormat::ModsToml)
    );
    assert_eq!(
        ModMetaFormat::from_archive_path("META-INF/neoforge.mods.toml"),
        Some(ModMetaFormat::NeoForgeModsToml)
    );
    assert_eq!(
        ModMetaFormat::from_archive_path("META-INF/MANIFEST.MF"),
        None
    );
}

#[test]
fn handles_quilt_depends_arrays_and_unknown_shapes() {
    let json = r#"{
        "schema_version": 1,
        "quilt_loader": {
            "id": "demo",
            "version": "${version}",
            "depends": [
                ["mod_a", {"id": "mod_b", "versions": ">=1.0"}],
                123,
                {"optional": true},
                {"id": "mod_c", "versions": {"unexpected": 1}}
            ]
        }
    }"#;
    let meta = parse_mod_meta(ModMetaFormat::QuiltModJson, json).unwrap();
    assert_eq!(meta.version, None);
    assert_eq!(
        meta.depends,
        vec![
            ModDependency {
                id: "mod_a".into(),
                version_req: "*".into(),
                required: false,
            },
            ModDependency {
                id: "mod_b".into(),
                version_req: ">=1.0".into(),
                required: false,
            },
            ModDependency {
                id: "mod_c".into(),
                version_req: "*".into(),
                required: true,
            },
        ]
    );
}

#[test]
fn handles_missing_version_and_rejects_broken_inputs() {
    let no_ver = parse_mod_meta(
        ModMetaFormat::FabricModJson,
        r#"{"id":"demo","version":"  "}"#,
    )
    .unwrap();
    assert_eq!(no_ver.version, None);

    assert!(matches!(
        parse_mod_meta(ModMetaFormat::FabricModJson, "{bad json"),
        Err(ModMetaError::Json(_))
    ));
    assert!(matches!(
        parse_mod_meta(ModMetaFormat::FabricModJson, r#"{"version":"1.0"}"#),
        Err(ModMetaError::MissingField("id"))
    ));
    assert!(matches!(
        parse_mod_meta(ModMetaFormat::QuiltModJson, "{}"),
        Err(ModMetaError::MissingField("quilt_loader"))
    ));
    assert!(matches!(
        parse_mod_meta(
            ModMetaFormat::QuiltModJson,
            r#"{"quilt_loader":{"version":"1.0"}}"#
        ),
        Err(ModMetaError::MissingField("id"))
    ));
    assert!(matches!(
        parse_mod_meta(ModMetaFormat::ModsToml, "not = [toml"),
        Err(ModMetaError::Toml(_))
    ));
    assert!(matches!(
        parse_mod_meta(ModMetaFormat::ModsToml, "modLoader = \"javafml\""),
        Err(ModMetaError::MissingField("mods"))
    ));
    assert!(matches!(
        parse_mod_meta(
            ModMetaFormat::NeoForgeModsToml,
            "[[mods]]\nversion = \"1.0\""
        ),
        Err(ModMetaError::MissingField("id"))
    ));
}
