//! Đọc ghi file người chơi. Không đụng mạng.

#![allow(clippy::unwrap_used)]

use std::fs;

use mh_core::PlayerName;
use mh_store::{BanEntry, OpEntry, PlayerFiles, PlayerRef, ServerStore, WhitelistEntry};
use serde_json::Value;

fn name(raw: &str) -> PlayerName {
    PlayerName::parse(raw).unwrap()
}

fn sample() -> (tempfile::TempDir, PlayerFiles) {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join("server")).unwrap();
    let files = PlayerFiles::from_root(tmp.path());
    (tmp, files)
}

#[test]
fn ops_round_trip_keeps_unknown_field_and_two_space_indent() {
    let (tmp, files) = sample();
    let raw = r#"[
  {
    "uuid": "b50ad385-829d-3141-a216-7e7d7539ba7f",
    "name": "Notch",
    "level": 4,
    "bypassesPlayerLimit": false,
    "extra": 1
  }
]"#;
    let path = tmp.path().join("server").join("ops.json");
    fs::write(&path, raw).unwrap();

    let ops = files.read_ops().unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].name.as_str(), "Notch");
    assert_eq!(ops[0].level, 4);
    assert!(!ops[0].bypasses_player_limit);
    assert_eq!(ops[0].extra.get("extra").and_then(Value::as_i64), Some(1));

    files.write_ops(&ops).unwrap();
    let again = files.read_ops().unwrap();
    assert_eq!(again, ops);

    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("\n  {\n    \"uuid\": "), "{text}");
    assert!(text.ends_with("]\n"), "{text}");
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value[0]["extra"], 1);
    assert_eq!(value[0]["name"], "Notch");
    assert_eq!(value[0]["level"], 4);
    assert_eq!(value[0]["bypassesPlayerLimit"], false);
}

#[test]
fn add_op_same_name_different_case_does_not_append() {
    let (_tmp, files) = sample();
    let uuid = "5627dd98-e6be-3c21-b8a8-e92344183641";
    assert!(
        files
            .add_op(OpEntry::new(uuid, name("Steve"), 4, false))
            .unwrap()
    );
    assert!(
        !files
            .add_op(OpEntry::new(
                "11111111-1111-1111-1111-111111111111",
                name("steve"),
                1,
                true,
            ))
            .unwrap()
    );

    let ops = files.read_ops().unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].name.as_str(), "Steve");
    assert_eq!(ops[0].uuid, uuid);
    assert_eq!(ops[0].level, 4);
    assert!(!ops[0].bypasses_player_limit);

    assert!(files.remove_op(PlayerRef::Name("STEVE")).unwrap());
    assert!(files.read_ops().unwrap().is_empty());
    assert!(!files.remove_op(PlayerRef::Name("Steve")).unwrap());
}

#[test]
fn missing_or_blank_player_files_are_empty() {
    let (tmp, files) = sample();
    assert!(files.read_ops().unwrap().is_empty());
    assert!(files.read_whitelist().unwrap().is_empty());
    assert!(files.read_bans().unwrap().is_empty());
    assert!(files.read_usercache().unwrap().is_empty());
    assert!(files.read_frozen().unwrap().is_empty());

    fs::write(tmp.path().join("server").join("ops.json"), "\n").unwrap();
    assert!(files.read_ops().unwrap().is_empty());
}

#[test]
fn usercache_reads_expires_on_as_text() {
    let (tmp, files) = sample();
    let raw = r#"[
  {
    "name": "Steve",
    "uuid": "5627dd98-e6be-3c21-b8a8-e92344183641",
    "expiresOn": "2026-11-26 10:00:00 +0700",
    "extra": 1
  }
]"#;
    fs::write(tmp.path().join("server").join("usercache.json"), raw).unwrap();

    let cache = files.read_usercache().unwrap();
    assert_eq!(cache.len(), 1);
    assert_eq!(cache[0].name.as_str(), "Steve");
    assert_eq!(cache[0].uuid, "5627dd98-e6be-3c21-b8a8-e92344183641");
    assert_eq!(cache[0].expires_on, "2026-11-26 10:00:00 +0700");
}

#[test]
fn whitelist_same_uuid_different_case_is_one_row() {
    let (_tmp, files) = sample();
    let uuid = "B50AD385-829D-3141-A216-7E7D7539BA7F";
    assert!(
        files
            .add_whitelist(WhitelistEntry::new(uuid, name("Notch")))
            .unwrap()
    );
    assert!(
        !files
            .add_whitelist(WhitelistEntry::new(
                uuid.to_ascii_lowercase(),
                name("Steve"),
            ))
            .unwrap()
    );
    let rows = files.read_whitelist().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name.as_str(), "Notch");
    assert!(
        files
            .remove_whitelist(PlayerRef::Uuid(&uuid.to_ascii_lowercase()))
            .unwrap()
    );
    assert!(files.read_whitelist().unwrap().is_empty());
}

#[test]
fn ban_round_trip_keeps_reason_and_unknown_field() {
    let (tmp, files) = sample();
    let raw = r#"[
  {
    "uuid": "5627dd98-e6be-3c21-b8a8-e92344183641",
    "name": "Steve",
    "created": "2026-10-27 10:00:00 +0700",
    "source": "Server",
    "expires": "forever",
    "reason": "Lý do",
    "extra": 1
  }
]"#;
    let path = tmp.path().join("server").join("banned-players.json");
    fs::write(&path, raw).unwrap();

    let bans = files.read_bans().unwrap();
    assert_eq!(bans.len(), 1);
    assert_eq!(bans[0].created, "2026-10-27 10:00:00 +0700");
    assert_eq!(bans[0].source, "Server");
    assert_eq!(bans[0].expires, "forever");
    assert_eq!(bans[0].reason, "Lý do");
    assert_eq!(bans[0].extra.get("extra").and_then(Value::as_i64), Some(1));

    files.write_bans(&bans).unwrap();
    assert_eq!(files.read_bans().unwrap(), bans);
    let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(value[0]["extra"], 1);
    assert_eq!(value[0]["reason"], "Lý do");

    assert!(files.remove_ban(PlayerRef::Name("steve")).unwrap());
    assert!(files.read_bans().unwrap().is_empty());
}

#[test]
fn frozen_list_lives_beside_server_dir_and_ignores_case() {
    let (tmp, files) = sample();
    assert!(!files.remove_frozen("Steve").unwrap());
    assert!(!tmp.path().join("frozen.json").exists());

    assert!(files.add_frozen(&name("Steve")).unwrap());
    assert!(!files.add_frozen(&name("steve")).unwrap());
    assert_eq!(files.read_frozen().unwrap(), vec![name("Steve")]);
    assert!(tmp.path().join("frozen.json").is_file());
    assert!(!tmp.path().join("server").join("frozen.json").exists());

    assert!(files.remove_frozen("STEVE").unwrap());
    assert!(files.read_frozen().unwrap().is_empty());
}

#[test]
fn store_players_writes_frozen_next_to_instance_toml() {
    let tmp = tempfile::tempdir().unwrap();
    let store = ServerStore::new(tmp.path().join("servers"), tmp.path().join("trash"));
    let id = mh_core::ServerId::generate();
    let paths = store.create_dir(id).unwrap();
    let files = store.players(id);

    assert!(files.add_frozen(&name("Alex")).unwrap());
    assert!(paths.root.join("frozen.json").is_file());
    assert!(!paths.server_dir.join("frozen.json").exists());
}

#[test]
fn invalid_ops_json_uses_config_invalid() {
    let (tmp, files) = sample();
    fs::write(tmp.path().join("server").join("ops.json"), "{").unwrap();
    let err = files.read_ops().unwrap_err();
    assert_eq!(err.code(), mh_core::ErrorCode::ConfigInvalid);
}

#[test]
fn add_ban_builds_a_row_without_dropping_fields() {
    let (_tmp, files) = sample();
    let entry = BanEntry::new(
        "5627dd98-e6be-3c21-b8a8-e92344183641",
        name("Steve"),
        "2026-10-27 10:00:00 +0700",
        "Server",
        "forever",
        "Lý do",
    );
    assert!(files.add_ban(entry).unwrap());
    let bans = files.read_bans().unwrap();
    assert_eq!(bans.len(), 1);
    assert_eq!(bans[0].reason, "Lý do");
    assert!(bans[0].extra.is_empty());
}
