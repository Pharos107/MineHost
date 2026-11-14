use super::*;

#[test]
fn plugin_yml_parses_inline_and_multiline_lists() {
    let yml = "\u{feff}name: \"DemoPlugin\"\r\nversion: '1.2.3' # comment\r\ndepend:\r\n  - Vault # inline comment\r\n  - 'ProtocolLib'\r\nsoftdepend: [\"PlaceholderAPI\", LuckPerms]\r\ncommands:\r\n  demo:\r\n    usage: |\r\n      /demo run\r\n";
    let meta = parse_plugin_yml(yml).unwrap();
    assert_eq!(meta.id, "DemoPlugin");
    assert_eq!(meta.name, "DemoPlugin");
    assert_eq!(meta.version.as_deref(), Some("1.2.3"));
    assert_eq!(meta.side, ModSide::Server);
    assert_eq!(
        meta.depends,
        vec![
            ModDependency {
                id: "Vault".into(),
                version_req: "*".into(),
                required: true,
            },
            ModDependency {
                id: "ProtocolLib".into(),
                version_req: "*".into(),
                required: true,
            },
            ModDependency {
                id: "PlaceholderAPI".into(),
                version_req: "*".into(),
                required: false,
            },
            ModDependency {
                id: "LuckPerms".into(),
                version_req: "*".into(),
                required: false,
            },
        ]
    );
}

#[test]
fn plugin_yml_handles_empty_flow_list_same_indent_and_placeholder_version() {
    let yml =
        "name: Demo\nversion: ${version}\ndepend: []\nsoftdepend:\n- Vault\nauthors:\n- Someone\n";
    let meta = parse_plugin_yml(yml).unwrap();
    assert_eq!(meta.version, None);
    assert_eq!(meta.depends.len(), 1);
    assert_eq!(meta.depends[0].id, "Vault");
    assert!(!meta.depends[0].required);
}

#[test]
fn paper_plugin_yml_parses_required_default_and_inline() {
    let yml = "\
name: MyPaper
version: 2.0.0
dependencies:
  bootstrap:
    IgnoredPlugin:
      required: true
  server:
    CoreLib:
      load: BEFORE
    OptApi:
      required: false
    InlineDep: { load: AFTER, required: false }
";
    let meta = parse_paper_plugin_yml(yml).unwrap();
    assert_eq!(meta.side, ModSide::Server);
    assert_eq!(meta.version.as_deref(), Some("2.0.0"));
    assert_eq!(
        meta.depends,
        vec![
            ModDependency {
                id: "CoreLib".into(),
                version_req: "*".into(),
                required: true,
            },
            ModDependency {
                id: "OptApi".into(),
                version_req: "*".into(),
                required: false,
            },
            ModDependency {
                id: "InlineDep".into(),
                version_req: "*".into(),
                required: false,
            },
        ]
    );
}

#[test]
fn yaml_rejects_malformed_and_missing_id() {
    assert!(matches!(
        parse_plugin_yml("not_a_key_value"),
        Err(ModMetaError::Yaml(_))
    ));
    assert!(matches!(
        parse_plugin_yml("name: A\nversion: 1\ndepend: [unclosed"),
        Err(ModMetaError::Yaml(_))
    ));
    assert!(matches!(
        parse_plugin_yml("name: \"unclosed\nversion: 1"),
        Err(ModMetaError::Yaml(_))
    ));
    assert!(matches!(
        parse_plugin_yml("name: Demo\n\tdepend: []"),
        Err(ModMetaError::Yaml(_))
    ));
    assert!(matches!(
        parse_plugin_yml("version: 1.0"),
        Err(ModMetaError::MissingField("id"))
    ));
    let no_ver = parse_paper_plugin_yml("name: Demo").unwrap();
    assert_eq!(no_ver.version, None);
}
