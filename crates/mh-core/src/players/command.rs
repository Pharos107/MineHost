//! Lệnh console cho từng thao tác ở spec P2, mục 6.7.

use time::OffsetDateTime;

use crate::error::{ErrorCode, InvalidValue};
use crate::players::PlayerName;
use crate::players::freeze::{self, FreezeAttribute};

/// Thao tác trên một người chơi. Lý do và chữ hiện lên màn hình kiểm tra lúc tạo lệnh,
/// không phải lúc tạo giá trị này.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerAction {
    WhitelistAdd,
    WhitelistRemove,
    Op,
    Deop,
    Kick {
        reason: String,
    },
    Ban {
        reason: String,
    },
    Pardon,
    Freeze,
    Unfreeze,
    TeleportToPlayer {
        target: PlayerName,
    },
    /// Điểm spawn trong overworld. App truyền tọa độ đọc từ `level.dat`.
    TeleportTo {
        x: f64,
        y: f64,
        z: f64,
    },
    Message {
        text: String,
    },
    Title {
        text: String,
    },
}

/// Lệnh không tạo được: chữ không an toàn để gửi stdin, hoặc bản Minecraft chưa có `attribute`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlayerCommandError {
    #[error(transparent)]
    Invalid(#[from] InvalidValue),
    #[error("phiên bản Minecraft chưa có lệnh attribute")]
    UnsupportedByVersion,
}

impl PlayerCommandError {
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Invalid(value) => value.code(),
            Self::UnsupportedByVersion => ErrorCode::UnsupportedByVersion,
        }
    }
}

impl From<freeze::UnsupportedVersion> for PlayerCommandError {
    fn from(_: freeze::UnsupportedVersion) -> Self {
        Self::UnsupportedByVersion
    }
}

/// Tạo một hoặc nhiều lệnh console.
///
/// `release_time` chỉ dùng cho đóng băng và bỏ đóng băng. Mỗi chuỗi là một lệnh, không có
/// ký tự xuống dòng: stdin của server coi mỗi dòng là một lệnh riêng.
pub fn command(
    action: &PlayerAction,
    player: &PlayerName,
    release_time: OffsetDateTime,
) -> Result<Vec<String>, PlayerCommandError> {
    let name = player.as_str();
    let lines = match action {
        PlayerAction::WhitelistAdd => vec![format!("whitelist add {name}")],
        PlayerAction::WhitelistRemove => vec![format!("whitelist remove {name}")],
        PlayerAction::Op => vec![format!("op {name}")],
        PlayerAction::Deop => vec![format!("deop {name}")],
        PlayerAction::Kick { reason } => vec![with_reason("kick", name, reason)?],
        PlayerAction::Ban { reason } => vec![with_reason("ban", name, reason)?],
        PlayerAction::Pardon => vec![format!("pardon {name}")],
        PlayerAction::Freeze => attribute_lines(player, release_time, false)?,
        PlayerAction::Unfreeze => attribute_lines(player, release_time, true)?,
        PlayerAction::TeleportToPlayer { target } => {
            vec![format!("tp {name} {}", target.as_str())]
        }
        PlayerAction::TeleportTo { x, y, z } => vec![teleport_spawn(name, *x, *y, *z)?],
        PlayerAction::Message { text } => vec![tellraw(name, text)?],
        PlayerAction::Title { text } => vec![title(name, text)?],
    };
    Ok(lines)
}

fn with_reason(verb: &str, name: &str, reason: &str) -> Result<String, InvalidValue> {
    reject_controls(reason, "Reason")?;
    if reason.is_empty() {
        Ok(format!("{verb} {name}"))
    } else {
        Ok(format!("{verb} {name} {reason}"))
    }
}

fn attribute_lines(
    player: &PlayerName,
    release_time: OffsetDateTime,
    restore_default: bool,
) -> Result<Vec<String>, PlayerCommandError> {
    let name = player.as_str();
    let attrs = freeze::freeze_attributes(release_time)?;
    Ok(attrs
        .iter()
        .map(|attr| attribute_line(name, attr, restore_default))
        .collect())
}

fn attribute_line(name: &str, attr: &FreezeAttribute, restore_default: bool) -> String {
    let value = if restore_default {
        attr.default_base
    } else {
        "0"
    };
    format!("attribute {name} {} base set {value}", attr.name)
}

/// Spec đóng điểm spawn trong overworld. `execute in` chọn chiều trước khi `tp`.
fn teleport_spawn(name: &str, x: f64, y: f64, z: f64) -> Result<String, InvalidValue> {
    if ![x, y, z].into_iter().all(f64::is_finite) {
        return Err(InvalidValue::new("Coordinate", "không phải số hữu hạn"));
    }
    Ok(format!(
        "execute in minecraft:overworld run tp {name} {x} {y} {z}"
    ))
}

fn tellraw(name: &str, text: &str) -> Result<String, InvalidValue> {
    Ok(format!("tellraw {name} {}", json_text(text)?))
}

fn title(name: &str, text: &str) -> Result<String, InvalidValue> {
    Ok(format!("title {name} title {}", json_text(text)?))
}

fn json_text(text: &str) -> Result<String, InvalidValue> {
    reject_controls(text, "Text")?;
    let mut out = String::from("{\"text\":");
    push_json_string(&mut out, text);
    out.push('}');
    Ok(out)
}

/// Xuống dòng trong stdin là một lệnh mới. Ký tự điều khiển khác cũng không được vào dòng lệnh.
/// U+2028/U+2029 không phải ký tự điều khiển nhưng có công cụ coi là xuống dòng, nên chặn luôn.
fn reject_controls(text: &str, kind: &'static str) -> Result<(), InvalidValue> {
    let breaks_line = |c: char| c.is_control() || c == '\u{2028}' || c == '\u{2029}';
    if text.chars().any(breaks_line) {
        Err(InvalidValue::new(
            kind,
            "có ký tự điều khiển hoặc xuống dòng",
        ))
    } else {
        Ok(())
    }
}

/// Component `{"text":…}`. Ký tự ngoài ASCII thành `\uXXXX` để cả lệnh là ASCII:
/// stdin trên Windows có thể không phải UTF-8, Minecraft tự giải mã JSON.
fn push_json_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_ascii() && !c.is_control() => out.push(c),
            c => push_u_escape(out, c),
        }
    }
    out.push('"');
}

fn push_u_escape(out: &mut String, ch: char) {
    let mut units = [0u16; 2];
    for unit in ch.encode_utf16(&mut units) {
        push_hex4(out, *unit);
    }
}

fn push_hex4(out: &mut String, unit: u16) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push_str("\\u");
    for shift in [12u16, 8, 4, 0] {
        let nibble = usize::from((unit >> shift) & 0x0f);
        out.push(char::from(HEX[nibble]));
    }
}
