use time::macros::datetime;

use super::*;

/// Ví dụ trong spec P1, mục 4.1, giữ nguyên comment và bảng nội tuyến.
const SPEC_EXAMPLE: &str = r#"
schema = 1
id = "01JB8Z6Q0K3V5T2N8XWJ4R7M9C"
name = "Server của tôi"
created_at = "2026-10-23T10:00:00Z"

[game]
loader = "paper"
minecraft = "1.21.4"
build = 232                      # build Paper đang dùng
jar = "server.jar"
java_major = 21

[runtime]
memory_mb = { min = 1024, max = 4096 }
jvm_args = []

[network]
port = 25565

[policy]
autostart = false
restart_on_crash = true

[eula]
accepted_at = "2026-10-23T10:05:00Z"
"#;

fn sample() -> InstanceSpec {
    let game = GameSection::new(
        LoaderKind::Paper,
        McVersion::parse("1.21.4").unwrap(),
        Some(232),
        21,
    );
    InstanceSpec::new(
        ServerId::parse("01JB8Z6Q0K3V5T2N8XWJ4R7M9C").unwrap(),
        ServerName::parse("Server của tôi").unwrap(),
        datetime!(2026-10-23 10:00 UTC),
        game,
        MemoryLimits::new(1024, 4096).unwrap(),
        GamePort::DEFAULT,
    )
}

#[test]
fn reads_spec_example() {
    let spec = InstanceSpec::from_toml(SPEC_EXAMPLE).unwrap();
    let mut expected = sample();
    expected.eula.accepted_at = Some(datetime!(2026-10-23 10:05 UTC));
    assert_eq!(spec, expected);
    assert!(spec.eula_accepted());
}

#[test]
fn writes_and_reads_back() {
    let spec = sample();
    let text = spec.to_toml().unwrap();
    assert!(text.starts_with("schema = 1\n"), "{text}");
    assert!(
        text.contains("created_at = \"2026-10-23T10:00:00Z\""),
        "{text}"
    );
    assert!(
        !text.contains("[eula]"),
        "chưa đồng ý thì không ghi bảng eula: {text}"
    );
    assert_eq!(InstanceSpec::from_toml(&text).unwrap(), spec);
}

#[test]
fn missing_optional_tables_use_defaults() {
    let text = r#"
schema = 1
id = "01JB8Z6Q0K3V5T2N8XWJ4R7M9C"
name = "A"
created_at = "2026-10-23T10:00:00Z"
[game]
loader = "paper"
minecraft = "1.21.4"
jar = "server.jar"
java_major = 21
[runtime]
memory_mb = { min = 1024, max = 1024 }
[network]
port = 25570
"#;
    let spec = InstanceSpec::from_toml(text).unwrap();
    assert!(!spec.policy.autostart);
    assert!(spec.policy.restart_on_crash);
    assert!(!spec.eula_accepted());
    assert_eq!(spec.game.build, None);
    assert!(spec.runtime.jvm_args.is_empty());
}

#[test]
fn keeps_unknown_keys_on_every_level() {
    // Khóa lạ ở cấp gốc phải đứng trước bảng đầu tiên, nếu không TOML hiểu là khóa của `[eula]`.
    let text = format!(
        "future_flag = true\n{SPEC_EXAMPLE}\n[future]\nlist = [1, 2]\nwhen = 2026-01-01T00:00:00Z\n"
    )
    .replace("[game]\n", "[game]\nmodpack = \"abc\"\n")
    .replace("[network]\n", "[network]\nbind = \"0.0.0.0\"\n")
    .replace("[eula]\n", "[eula]\nby = \"lehai\"\n");
    let spec = InstanceSpec::from_toml(&text).unwrap();
    assert_eq!(
        spec.extra.get("future_flag"),
        Some(&toml::Value::Boolean(true))
    );
    assert!(spec.extra.contains_key("future"));
    assert!(spec.game.extra.contains_key("modpack"));
    assert!(spec.network.extra.contains_key("bind"));
    assert!(spec.eula.extra.contains_key("by"));

    let again = InstanceSpec::from_toml(&spec.to_toml().unwrap()).unwrap();
    assert_eq!(again, spec);
    let future = again.extra["future"].as_table().unwrap();
    assert_eq!(future["list"].as_array().unwrap().len(), 2);
    assert!(future["when"].as_datetime().is_some());
}

#[test]
fn newer_schema_is_reported_before_other_errors() {
    // Bản mới hơn có thể đổi hẳn cấu trúc, nên phải báo đúng lý do thay vì lỗi parse.
    let err = InstanceSpec::from_toml("schema = 2\nid = 42\n").unwrap_err();
    assert!(
        matches!(err, InstanceError::NewerSchema { found: 2 }),
        "{err:?}"
    );
    assert_eq!(err.code(), ErrorCode::ConfigInvalid);
}

#[test]
fn missing_or_old_schema_is_rejected() {
    let without = SPEC_EXAMPLE.replace("schema = 1\n", "");
    for text in [without, SPEC_EXAMPLE.replace("schema = 1", "schema = 0")] {
        let err = InstanceSpec::from_toml(&text).unwrap_err();
        assert!(matches!(err, InstanceError::BadSchema), "{err:?}");
    }
}

#[test]
fn invalid_values_are_rejected() {
    let cases = [
        ("port = 25565", "port = 80"),
        ("name = \"Server của tôi\"", "name = \"  \""),
        ("id = \"01JB8Z6Q0K3V5T2N8XWJ4R7M9C\"", "id = \"../x\""),
        ("min = 1024, max = 4096", "min = 4096, max = 1024"),
        ("loader = \"paper\"", "loader = \"forge\""),
        ("jar = \"server.jar\"", "jar = \"../server.jar\""),
        (
            "created_at = \"2026-10-23T10:00:00Z\"",
            "created_at = \"hôm qua\"",
        ),
        ("[network]\nport = 25565\n", ""),
        ("this is not toml", "this is not toml"),
    ];
    for (from, to) in cases {
        let text = if from == to {
            from.to_owned()
        } else {
            SPEC_EXAMPLE.replace(from, to)
        };
        let err = InstanceSpec::from_toml(&text).unwrap_err();
        assert!(matches!(err, InstanceError::Parse(_)), "{to}: {err:?}");
        assert_eq!(err.code(), ErrorCode::ConfigInvalid);
    }
}

#[test]
fn jar_name_is_a_plain_file_name() {
    for ok in ["server.jar", "paper-1.21.4-232.jar", "a_b.jar"] {
        assert_eq!(JarName::parse(ok).unwrap().as_str(), ok);
    }
    for bad in [
        "",
        ".jar",
        "server",
        "../server.jar",
        "a/b.jar",
        "a\\b.jar",
        ".hidden.jar",
    ] {
        assert!(JarName::parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn restart_at_round_trips_and_is_omitted_when_empty() {
    let text = SPEC_EXAMPLE.replace(
        "restart_on_crash = true\n",
        "restart_on_crash = true\nrestart_at = [\"04:00\", \"16:30\"]\n",
    );
    let spec = InstanceSpec::from_toml(&text).unwrap();
    let times: Vec<String> = spec
        .policy
        .restart_at
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(times, ["04:00", "16:30"]);
    assert!(
        spec.to_toml()
            .unwrap()
            .contains("restart_at = [\"04:00\", \"16:30\"]")
    );

    // Bản P1 không có khóa này: đọc và ghi lại không thêm gì.
    assert!(!sample().to_toml().unwrap().contains("restart_at"));
    let bad = SPEC_EXAMPLE.replace(
        "restart_on_crash = true\n",
        "restart_on_crash = true\nrestart_at = [\"4:00\"]\n",
    );
    assert!(InstanceSpec::from_toml(&bad).is_err());
}
