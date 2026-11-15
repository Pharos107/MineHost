//! Bảng `instance.toml` thêm ở P3 (spec P3, mục 4.1).

use super::*;
use crate::launch::ArgsFilePath;

const NEOFORGE: &str = r#"
schema = 1
id = "01JB8Z6Q0K3V5T2N8XWJ4R7M9C"
name = "Có mod"
created_at = "2026-11-13T10:00:00Z"

[game]
loader = "neoforge"
minecraft = "1.21.4"
loader_version = "21.4.140"
java_major = 21
jar = "server.jar"

[game.launch]
kind = "args_file"
args_file = "libraries/net/neoforged/neoforge/21.4.140/{os}_args.txt"
future_key = 1

[runtime]
memory_mb = { min = 1024, max = 6144 }

[network]
port = 25565

[modpack]
source = "modrinth"
project_id = "1KVo5zza"
version_id = "aBcD1234"
name = "Fabulously Optimized"
version_name = "6.4.0"
"#;

#[test]
fn reads_mod_loader_server() {
    let spec = InstanceSpec::from_toml(NEOFORGE).unwrap();
    assert_eq!(spec.game.loader, LoaderKind::NeoForge);
    assert_eq!(
        spec.game.loader_version.as_ref().map(LoaderVersion::as_str),
        Some("21.4.140")
    );
    let Launch::ArgsFile(path) = spec.game.launch() else {
        panic!("phải là args_file");
    };
    assert_eq!(
        path.as_str(),
        "libraries/net/neoforged/neoforge/21.4.140/{os}_args.txt"
    );
    let pack = spec.modpack.as_ref().unwrap();
    assert_eq!(pack.source, ModpackSource::Modrinth);
    assert_eq!(pack.name.as_str(), "Fabulously Optimized");
}

#[test]
fn round_trip_keeps_new_tables_and_unknown_keys() {
    let spec = InstanceSpec::from_toml(NEOFORGE).unwrap();
    let text = spec.to_toml().unwrap();
    assert!(text.contains("future_key = 1"), "{text}");
    assert!(text.contains("kind = \"args_file\""), "{text}");
    assert_eq!(InstanceSpec::from_toml(&text).unwrap(), spec);
}

#[test]
fn p2_file_launches_with_jar() {
    let game = GameSection::new(
        LoaderKind::Paper,
        McVersion::parse("1.21.4").unwrap(),
        Some(232),
        21,
    );
    assert_eq!(game.launch(), Launch::Jar(JarName::server_jar()));
    let explicit = GameSection {
        launch: Some(LaunchSection::new(LaunchMode::Jar)),
        ..game.clone()
    };
    assert_eq!(explicit.launch(), Launch::Jar(JarName::server_jar()));
}

#[test]
fn new_fields_are_omitted_when_absent() {
    let game = GameSection::new(
        LoaderKind::Fabric,
        McVersion::parse("1.21.4").unwrap(),
        None,
        21,
    );
    let text = toml::to_string(&game).unwrap();
    assert!(!text.contains("launch"), "{text}");
    assert!(!text.contains("loader_version"), "{text}");
}

#[test]
fn launch_table_must_be_consistent() {
    let missing = NEOFORGE.replace(
        "args_file = \"libraries/net/neoforged/neoforge/21.4.140/{os}_args.txt\"\n",
        "",
    );
    assert!(InstanceSpec::from_toml(&missing).is_err());
    let escaping = NEOFORGE.replace(
        "libraries/net/neoforged/neoforge/21.4.140/{os}_args.txt",
        "../../evil.txt",
    );
    assert!(InstanceSpec::from_toml(&escaping).is_err());
    let jar_with_file = NEOFORGE.replace("kind = \"args_file\"", "kind = \"jar\"");
    assert!(InstanceSpec::from_toml(&jar_with_file).is_err());
    let unknown = NEOFORGE.replace("kind = \"args_file\"", "kind = \"script\"");
    assert!(InstanceSpec::from_toml(&unknown).is_err());
}

#[test]
fn modpack_values_are_checked() {
    let bad_id = NEOFORGE.replace("\"1KVo5zza\"", "\"../x\"");
    assert!(InstanceSpec::from_toml(&bad_id).is_err());
    let control = NEOFORGE.replace("Fabulously Optimized", "Pack\\u0007");
    assert!(InstanceSpec::from_toml(&control).is_err());
    let file_pack = NEOFORGE
        .replace("source = \"modrinth\"", "source = \"file\"")
        .replace("project_id = \"1KVo5zza\"\n", "")
        .replace("version_id = \"aBcD1234\"\n", "");
    let spec = InstanceSpec::from_toml(&file_pack).unwrap();
    let pack = spec.modpack.unwrap();
    assert_eq!(pack.source, ModpackSource::File);
    assert!(pack.project_id.is_none());
}

#[test]
fn upstream_text_is_trimmed_and_bounded() {
    assert_eq!(UpstreamText::parse("  Pack  ").unwrap().as_str(), "Pack");
    assert!(UpstreamText::parse("   ").is_err());
    assert!(UpstreamText::parse(&"á".repeat(201)).is_err());
    assert!(ContentId::parse("238222").is_ok());
    assert!(ContentId::parse("").is_err());
    assert!(ArgsFilePath::parse("a/{os}_args.txt").is_ok());
}
