//! `config/fabric_loader_dependencies.json`: ghi đè dependency mà Fabric loader áp dụng.

#![allow(clippy::unwrap_used)]

use mh_core::{ModMetaFormat, parse_fabric_dep_overrides, parse_mod_meta};

fn ids(deps: &[mh_core::ModDependency]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = deps
        .iter()
        .map(|d| (d.id.clone(), d.version_req.clone()))
        .collect();
    out.sort();
    out
}

#[test]
fn removes_adds_and_replaces_like_fabric_loader() {
    let overrides = parse_fabric_dep_overrides(
        r#"{
        "version": 1,
        "overrides": {
            "ForgeConfigScreens": { "-depends": { "modmenu": "IGNORED" } },
            "cynosure": { "+depends": { "fabric-language-kotlin": "*", "lib": ">=2" } },
            "whole": { "depends": { "only": "1.x", "minecraft": "*" } },
            "broken": "not an object"
        }
    }"#,
    )
    .unwrap();

    let screens = parse_mod_meta(
        ModMetaFormat::FabricModJson,
        r#"{"id":"forgeconfigscreens","depends":{"modmenu":"*","fabric":"*"}}"#,
    )
    .unwrap();
    assert_eq!(
        ids(&overrides.effective_depends(&screens)),
        vec![("fabric".into(), "*".into())]
    );

    let cyno = parse_mod_meta(
        ModMetaFormat::FabricModJson,
        r#"{"id":"cynosure","depends":{"lib":">=1"}}"#,
    )
    .unwrap();
    assert_eq!(
        ids(&overrides.effective_depends(&cyno)),
        vec![
            ("fabric-language-kotlin".into(), "*".into()),
            ("lib".into(), ">=2".into())
        ]
    );

    let whole = parse_mod_meta(
        ModMetaFormat::FabricModJson,
        r#"{"id":"whole","depends":{"a":"*","b":"*"}}"#,
    )
    .unwrap();
    assert_eq!(
        ids(&overrides.effective_depends(&whole)),
        vec![("only".into(), "1.x".into())],
        "thay toàn bộ, vẫn lọc id nền tảng"
    );

    let other = parse_mod_meta(
        ModMetaFormat::FabricModJson,
        r#"{"id":"other","depends":{"x":"*"}}"#,
    )
    .unwrap();
    assert_eq!(overrides.effective_depends(&other), other.depends);
}

#[test]
fn rejects_unknown_version_and_bad_json() {
    assert!(parse_fabric_dep_overrides(r#"{"version":2,"overrides":{}}"#).is_err());
    assert!(parse_fabric_dep_overrides("{").is_err());
    let empty = parse_fabric_dep_overrides(r#"{"version":1}"#).unwrap();
    assert_eq!(empty, Default::default());
}
