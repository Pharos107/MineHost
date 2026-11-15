//! Kiểm thử tích hợp parser metadata trên 5 fixture plugin thật từ Modrinth.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use mh_core::{ModDependency, ModMetaError, ModMetaFormat, ModSide, parse_mod_meta};

struct ExpectedPlugin {
    dir: &'static str,
    file: &'static str,
    format: ModMetaFormat,
    id: &'static str,
    version: Option<&'static str>,
    key_deps: &'static [(&'static str, bool)],
}

const PLUGIN_FIXTURES: &[ExpectedPlugin] = &[
    ExpectedPlugin {
        dir: "worldguard",
        file: "plugin.yml",
        format: ModMetaFormat::PluginYml,
        id: "WorldGuard",
        version: Some("7.0.21+2424-da30a99"),
        key_deps: &[("WorldEdit", true)],
    },
    ExpectedPlugin {
        dir: "luckperms",
        file: "plugin.yml",
        format: ModMetaFormat::PluginYml,
        id: "LuckPerms",
        version: Some("5.5.71"),
        key_deps: &[("LilyPad-Connect", false), ("ViaVersion", false)],
    },
    ExpectedPlugin {
        dir: "vaultunlocked",
        file: "plugin.yml",
        format: ModMetaFormat::PluginYml,
        id: "Vault",
        version: Some("2.20.3"),
        key_deps: &[("PlaceholderAPI", false)],
    },
    ExpectedPlugin {
        dir: "chunky",
        file: "paper-plugin.yml",
        format: ModMetaFormat::PaperPluginYml,
        id: "Chunky",
        version: Some("1.5.66"),
        key_deps: &[
            ("dynmap", false),
            ("BlueMap", false),
            ("squaremap", false),
            ("WorldBorder", false),
        ],
    },
    ExpectedPlugin {
        dir: "fancyholograms",
        file: "paper-plugin.yml",
        format: ModMetaFormat::PaperPluginYml,
        id: "FancyHolograms",
        version: Some("3.0.0-SNAPSHOT.95"),
        key_deps: &[
            ("FancyNpcs", false),
            ("MiniPlaceholders", false),
            ("PlaceholderAPI", false),
            ("ViaVersion", false),
        ],
    },
];

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/meta")
}

#[test]
fn parses_all_real_plugin_fixtures() {
    let root = fixture_root();
    let mut seen_dirs = BTreeSet::new();

    for exp in PLUGIN_FIXTURES {
        let path = root.join(exp.dir).join(exp.file);
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        let meta = parse_mod_meta(exp.format, &text)
            .unwrap_or_else(|e| panic!("{} parse thất bại: {e}", exp.dir));
        assert_eq!(meta.id, exp.id, "{}", exp.dir);
        assert_eq!(meta.name, exp.id, "{}", exp.dir);
        assert_eq!(meta.version.as_deref(), exp.version, "{}", exp.dir);
        assert_eq!(meta.side, ModSide::Server, "{}", exp.dir);
        for &(dep_id, required) in exp.key_deps {
            let want = ModDependency {
                id: dep_id.to_owned(),
                version_req: "*".to_owned(),
                required,
            };
            assert!(meta.depends.contains(&want), "{} thiếu {want:?}", exp.dir);
        }
        assert!(seen_dirs.insert(exp.dir));
    }

    assert_eq!(PLUGIN_FIXTURES.len(), 5);
    let actual_dirs: BTreeSet<String> = fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(actual_dirs.len(), 25);
    assert!(seen_dirs.iter().all(|d| actual_dirs.contains(*d)));
}

#[test]
fn recognizes_plugin_archive_paths_and_rejects_broken_yaml() {
    assert_eq!(
        ModMetaFormat::from_archive_path("plugin.yml"),
        Some(ModMetaFormat::PluginYml)
    );
    assert_eq!(
        ModMetaFormat::from_archive_path("paper-plugin.yml"),
        Some(ModMetaFormat::PaperPluginYml)
    );
    assert!(matches!(
        parse_mod_meta(ModMetaFormat::PluginYml, "just text without colon"),
        Err(ModMetaError::Yaml(_))
    ));
    let paper_no_ver = parse_mod_meta(ModMetaFormat::PaperPluginYml, "name: Demo").unwrap();
    assert_eq!(paper_no_ver.version, None);
}
