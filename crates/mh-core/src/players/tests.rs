use time::OffsetDateTime;
use time::macros::datetime;

use crate::error::ErrorCode;

use super::*;

const VANILLA_1_16_5: &str = include_str!("../../fixtures/players/1.16.5.log");
const VANILLA_1_20_4: &str = include_str!("../../fixtures/players/1.20.4.log");
const VANILLA_1_21_4: &str = include_str!("../../fixtures/players/1.21.4.log");
const PAPER: &str = include_str!("../../fixtures/players/paper.log");

fn notch() -> PlayerName {
    PlayerName::parse("Notch").unwrap()
}

fn steve() -> PlayerName {
    PlayerName::parse("Steve").unwrap()
}

fn release() -> OffsetDateTime {
    datetime!(2021-01-01 0:00 UTC)
}

fn log_lines(raw: &str) -> Vec<&str> {
    raw.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .collect()
}

fn assert_log_fixture(raw: &str) {
    let lines = log_lines(raw);
    assert_eq!(lines.len(), 5);
    assert!(lines[4].contains("joined the game"), "{raw}");
    assert_eq!(
        parse_log_line(lines[0]),
        Some(PlayerLogEvent::Joined(notch()))
    );
    assert_eq!(
        parse_log_line(lines[1]),
        Some(PlayerLogEvent::Left(notch()))
    );
    assert_eq!(
        parse_log_line(lines[2]),
        Some(PlayerLogEvent::List {
            online: 0,
            max: 20,
            names: vec![]
        })
    );
    assert_eq!(
        parse_log_line(lines[3]),
        Some(PlayerLogEvent::List {
            online: 2,
            max: 20,
            names: vec![notch(), steve()]
        })
    );
    assert_eq!(parse_log_line(lines[4]), None);
}

#[test]
fn log_fixtures_of_each_era() {
    for raw in [VANILLA_1_16_5, VANILLA_1_20_4, VANILLA_1_21_4, PAPER] {
        assert_log_fixture(raw);
    }
}

#[test]
fn log_line_ignores_chat_and_near_misses() {
    let chat = [
        "<Notch> joined the game",
        "[12:00:00] [Server thread/INFO]: <Notch> left the game",
        "[12:00:00 INFO]: <Steve> I joined the game",
        "Notch joined the game today",
        "[Notch] joined the game",
    ];
    for line in chat {
        assert_eq!(parse_log_line(line), None, "{line}");
    }
}

#[test]
fn list_line_accepts_empty_with_or_without_trailing_space() {
    let empty = PlayerLogEvent::List {
        online: 0,
        max: 20,
        names: vec![],
    };
    assert_eq!(
        parse_log_line("There are 0 of a max of 20 players online:"),
        Some(empty.clone())
    );
    assert_eq!(
        parse_log_line("There are 0 of a max of 20 players online: "),
        Some(empty)
    );
    assert_eq!(
        parse_log_line("There are 3 of a max of 20 players online: Notch, Steve, Alex"),
        Some(PlayerLogEvent::List {
            online: 3,
            max: 20,
            names: vec![notch(), steve(), PlayerName::parse("Alex").unwrap(),]
        })
    );
    // `list uuids` và tên cụt không được cắt thành một phần danh sách.
    assert_eq!(
        parse_log_line("There are 1 of a max of 20 players online: Notch (b50ad385)"),
        None
    );
    assert_eq!(
        parse_log_line("There are 2 of a max of 20 players online: Notch, no"),
        None
    );
}

#[test]
fn joined_name_respects_length_limit() {
    let ok = "[12:00:00] [Server thread/INFO]: Abcdefghijklmnop joined the game";
    let too_long = "[12:00:00] [Server thread/INFO]: Abcdefghijklmnopq joined the game";
    assert!(matches!(
        parse_log_line(ok),
        Some(PlayerLogEvent::Joined(_))
    ));
    assert_eq!(parse_log_line(too_long), None);
}

#[test]
fn player_name_rejects_bad_shape_including_serde() {
    assert!(PlayerName::parse("Abc").is_ok());
    assert!(PlayerName::parse("Abcdefghijklmnop").is_ok());
    assert!(PlayerName::parse("ok_name").is_ok());
    for bad in [
        "ab",
        "Abcdefghijklmnopq",
        "has space",
        "tên",
        "no-hyphen",
        "a.b",
        "",
    ] {
        assert!(PlayerName::parse(bad).is_err(), "{bad}");
        let json = format!("\"{bad}\"");
        assert!(serde_json::from_str::<PlayerName>(&json).is_err(), "{bad}");
    }
    let name = notch();
    assert_eq!(serde_json::to_string(&name).unwrap(), "\"Notch\"");
    let back: PlayerName = serde_json::from_str("\"Notch\"").unwrap();
    assert_eq!(back, name);
}

#[test]
fn offline_uuid_matches_minecraft() {
    assert_eq!(
        offline_uuid(&notch()),
        "b50ad385-829d-3141-a216-7e7d7539ba7f"
    );
    assert_eq!(
        offline_uuid(&steve()),
        "5627dd98-e6be-3c21-b8a8-e92344183641"
    );
}

#[test]
fn tellraw_and_title_escape_json_as_ascii() {
    let text = "a\"b\\ chào";
    let message = command(
        &PlayerAction::Message { text: text.into() },
        &notch(),
        release(),
    )
    .unwrap();
    assert_eq!(message.len(), 1);
    assert_eq!(
        message[0],
        "tellraw Notch {\"text\":\"a\\\"b\\\\ ch\\u00e0o\"}"
    );
    assert!(message[0].is_ascii());
    assert_eq!(json_text_of(&message[0], "tellraw Notch "), text);

    let title = command(
        &PlayerAction::Title {
            text: "Xin chào 😀".into(),
        },
        &notch(),
        release(),
    )
    .unwrap();
    assert!(title[0].is_ascii());
    assert!(title[0].starts_with("title Notch title "));
    assert_eq!(json_text_of(&title[0], "title Notch title "), "Xin chào 😀");
}

fn json_text_of(command_line: &str, prefix: &str) -> String {
    let json = command_line.strip_prefix(prefix).unwrap();
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    value["text"].as_str().unwrap().to_owned()
}

#[test]
fn control_characters_in_reason_and_text_are_rejected() {
    for text in ["bad\nstop", "bad\rstop", "bad\tstop", "bad\u{2028}stop"] {
        let kick = command(
            &PlayerAction::Kick {
                reason: text.into(),
            },
            &notch(),
            release(),
        );
        assert!(kick.is_err(), "{text:?}");
        assert_eq!(kick.unwrap_err().code(), ErrorCode::InvalidInput);
        let message = command(
            &PlayerAction::Message { text: text.into() },
            &notch(),
            release(),
        );
        assert_eq!(message.unwrap_err().code(), ErrorCode::InvalidInput);
        let title = command(
            &PlayerAction::Title { text: text.into() },
            &notch(),
            release(),
        );
        assert_eq!(title.unwrap_err().code(), ErrorCode::InvalidInput);
    }
}

#[test]
fn plain_commands_keep_reason_text_unescaped() {
    let cases = [
        (PlayerAction::WhitelistAdd, "whitelist add Notch"),
        (PlayerAction::WhitelistRemove, "whitelist remove Notch"),
        (PlayerAction::Op, "op Notch"),
        (PlayerAction::Deop, "deop Notch"),
        (PlayerAction::Pardon, "pardon Notch"),
        (
            PlayerAction::Kick {
                reason: "said \"hi\"".into(),
            },
            "kick Notch said \"hi\"",
        ),
        (
            PlayerAction::Ban {
                reason: "a\\b".into(),
            },
            "ban Notch a\\b",
        ),
        (
            PlayerAction::Kick {
                reason: String::new(),
            },
            "kick Notch",
        ),
        (
            PlayerAction::TeleportToPlayer { target: steve() },
            "tp Notch Steve",
        ),
    ];
    for (action, expected) in cases {
        let lines = command(&action, &notch(), release()).unwrap();
        assert_eq!(lines, vec![expected.to_owned()]);
        assert!(!lines[0].chars().any(char::is_control));
    }
}

#[test]
fn teleport_to_spawn_runs_in_overworld() {
    let lines = command(
        &PlayerAction::TeleportTo {
            x: 1.5,
            y: 64.0,
            z: -20.0,
        },
        &notch(),
        release(),
    )
    .unwrap();
    assert_eq!(
        lines,
        vec!["execute in minecraft:overworld run tp Notch 1.5 64 -20".to_owned()]
    );
    let bad = command(
        &PlayerAction::TeleportTo {
            x: f64::NAN,
            y: 64.0,
            z: 0.0,
        },
        &notch(),
        release(),
    );
    assert_eq!(bad.unwrap_err().code(), ErrorCode::InvalidInput);
}

#[test]
fn freeze_attributes_follow_release_day() {
    let err = freeze_attributes(datetime!(2020-01-01 0:00 UTC)).unwrap_err();
    assert_eq!(err.code(), ErrorCode::UnsupportedByVersion);

    assert_eq!(
        freeze_attributes(datetime!(2021-01-01 0:00 UTC)).unwrap(),
        vec![speed(true)]
    );
    assert_eq!(
        freeze_attributes(datetime!(2024-06-01 0:00 UTC)).unwrap(),
        vec![speed(true), jump(true)]
    );
    assert_eq!(
        freeze_attributes(datetime!(2025-01-01 0:00 UTC)).unwrap(),
        vec![speed(false), jump(false)]
    );
}

#[test]
fn freeze_attributes_change_on_the_release_instant() {
    assert!(freeze_attributes(datetime!(2020-06-22 23:59 UTC)).is_err());
    assert_eq!(
        freeze_attributes(datetime!(2020-06-23 10:26 UTC)).unwrap(),
        vec![speed(true)]
    );
    assert_eq!(
        freeze_attributes(datetime!(2024-04-22 23:59 UTC)).unwrap(),
        vec![speed(true)]
    );
    assert_eq!(
        freeze_attributes(datetime!(2024-04-23 0:00 UTC)).unwrap(),
        vec![speed(true), jump(true)]
    );
    assert_eq!(
        freeze_attributes(datetime!(2024-10-21 23:59 UTC)).unwrap(),
        vec![speed(true), jump(true)]
    );
    assert_eq!(
        freeze_attributes(datetime!(2024-10-22 12:00 UTC)).unwrap(),
        vec![speed(false), jump(false)]
    );
}

fn speed(generic_prefix: bool) -> FreezeAttribute {
    FreezeAttribute {
        name: if generic_prefix {
            "generic.movement_speed"
        } else {
            "movement_speed"
        },
        default_base: "0.1",
    }
}

fn jump(generic_prefix: bool) -> FreezeAttribute {
    FreezeAttribute {
        name: if generic_prefix {
            "generic.jump_strength"
        } else {
            "jump_strength"
        },
        default_base: "0.42",
    }
}

#[test]
fn freeze_and_unfreeze_emit_one_command_per_attribute() {
    let frozen = command(
        &PlayerAction::Freeze,
        &notch(),
        datetime!(2024-06-01 0:00 UTC),
    )
    .unwrap();
    assert_eq!(
        frozen,
        vec![
            "attribute Notch generic.movement_speed base set 0".to_owned(),
            "attribute Notch generic.jump_strength base set 0".to_owned(),
        ]
    );
    let thawed = command(
        &PlayerAction::Unfreeze,
        &notch(),
        datetime!(2025-01-01 0:00 UTC),
    )
    .unwrap();
    assert_eq!(
        thawed,
        vec![
            "attribute Notch movement_speed base set 0.1".to_owned(),
            "attribute Notch jump_strength base set 0.42".to_owned(),
        ]
    );
    let err = command(
        &PlayerAction::Freeze,
        &notch(),
        datetime!(2020-01-01 0:00 UTC),
    )
    .unwrap_err();
    assert_eq!(err.code(), ErrorCode::UnsupportedByVersion);
    assert!(matches!(err, PlayerCommandError::UnsupportedByVersion));
}
