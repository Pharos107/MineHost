//! `provides` và jar lồng trong metadata mod (preflight tính chúng là mod đã có).

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;

use mh_core::{ModMetaFormat, parse_jarjar_metadata, parse_mod_meta};

fn fixture(dir: &str, file: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/meta")
        .join(dir)
        .join(file);
    fs::read_to_string(path).unwrap()
}

#[test]
fn fabric_api_provides_fabric_and_lists_nested_jars() {
    let text = fixture("fabric-api", "fabric.mod.json");
    let meta = parse_mod_meta(ModMetaFormat::FabricModJson, &text).unwrap();
    assert_eq!(meta.provides, vec!["fabric".to_owned()]);
    assert_eq!(meta.nested_jars.len(), 49);
    assert!(
        meta.nested_jars
            .contains(&"META-INF/jars/fabric-api-base-0.116.18.jar".to_owned())
    );
}

#[test]
fn sodium_provides_indium() {
    let text = fixture("sodium", "fabric.mod.json");
    let meta = parse_mod_meta(ModMetaFormat::FabricModJson, &text).unwrap();
    assert_eq!(meta.provides, vec!["indium".to_owned()]);
    assert!(!meta.nested_jars.is_empty());
}

#[test]
fn quilt_provides_objects_and_string_jars() {
    let text = fixture("qsl", "quilt.mod.json");
    let meta = parse_mod_meta(ModMetaFormat::QuiltModJson, &text).unwrap();
    assert_eq!(
        meta.provides,
        vec!["fabric-api".to_owned(), "fabric".to_owned()]
    );
    assert_eq!(meta.nested_jars.len(), 87);
    assert_eq!(
        meta.nested_jars[0],
        "META-INF/jars/item_setting-6.3.0+1.20.1.jar"
    );
}

#[test]
fn forge_toml_provides() {
    let text = fixture("oculus", "mods.toml");
    let meta = parse_mod_meta(ModMetaFormat::ModsToml, &text).unwrap();
    assert_eq!(meta.provides, vec!["iris".to_owned()]);
    assert!(meta.nested_jars.is_empty());
}

#[test]
fn odd_shapes_are_skipped() {
    let text = r#"{"schemaVersion":1,"id":"x","provides":["", 3, {"id":" y "}],
        "jars":[{"file":""}, 7, {"nope":1}, "  z.jar "]}"#;
    let meta = parse_mod_meta(ModMetaFormat::FabricModJson, text).unwrap();
    assert_eq!(meta.provides, vec!["y".to_owned()]);
    assert_eq!(meta.nested_jars, vec!["z.jar".to_owned()]);

    let bare = parse_mod_meta(ModMetaFormat::FabricModJson, r#"{"id":"x","provides":"y"}"#);
    assert!(bare.unwrap().provides.is_empty());
}

#[test]
fn reads_forge_jarjar_metadata() {
    let text = r#"{"jars":[
        {"identifier":{"group":"a","artifact":"b"},"version":{"range":"[1,)","artifactVersion":"1.0"},
         "path":"META-INF/jarjar/b-1.0.jar","isObfuscated":false},
        {"identifier":{"group":"c","artifact":"d"}},
        {"path":"  "}
    ]}"#;
    assert_eq!(
        parse_jarjar_metadata(text).unwrap(),
        vec!["META-INF/jarjar/b-1.0.jar".to_owned()]
    );
    assert!(parse_jarjar_metadata("{}").unwrap().is_empty());
    assert!(parse_jarjar_metadata("not json").is_err());
}

#[test]
fn accepts_raw_newlines_in_strings_like_fabric_loader() {
    let text = "{
  \"schemaVersion\": 1,
  \"id\": \"betterend\",
  \"version\": \"4.0.11\",
  \"description\": \"More content
Made by:
- paulevs\"
}";
    let meta = parse_mod_meta(ModMetaFormat::FabricModJson, text).unwrap();
    assert_eq!(meta.id, "betterend");
    assert_eq!(meta.version.as_deref(), Some("4.0.11"));
    assert!(parse_mod_meta(ModMetaFormat::FabricModJson, "{\"id\": ").is_err());
}

/// Forge/NeoForge không ép dependency `side = "CLIENT"` trên server (ATM10 cần `iris` chỉ ở client).
#[test]
fn client_side_forge_dependency_is_not_required_on_server() {
    let toml = r#"
[[mods]]
modId = "shaderbridge"
[[dependencies.shaderbridge]]
modId = "iris"
type = "required"
side = "CLIENT"
[[dependencies.shaderbridge]]
modId = "curios"
mandatory = true
side = "BOTH"
[[dependencies.shaderbridge]]
modId = "serverlib"
type = "required"
side = "SERVER"
[[dependencies.shaderbridge]]
modId = "oculus"
mandatory = true
side = " client "
"#;
    let meta = parse_mod_meta(ModMetaFormat::NeoForgeModsToml, toml).unwrap();
    let required: Vec<&str> = meta
        .depends
        .iter()
        .filter(|d| d.required)
        .map(|d| d.id.as_str())
        .collect();
    assert_eq!(required, vec!["curios", "serverlib"]);
    assert!(meta.depends.iter().any(|d| d.id == "iris" && !d.required));
}
