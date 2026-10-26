//! Một dòng log server thành sự kiện người chơi.

use crate::players::PlayerName;

/// Sự kiện đọc được từ một dòng log. `None` từ [`parse_log_line`] nghĩa là dòng không phải
/// join, leave hay `list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerLogEvent {
    Joined(PlayerName),
    Left(PlayerName),
    List {
        online: u32,
        max: u32,
        names: Vec<PlayerName>,
    },
}

/// Đọc một dòng log Vanilla hoặc Paper.
///
/// Chấp nhận tiền tố `[12:34:56] [Server thread/INFO]: ` và `[12:34:56 INFO]: `.
/// Dòng chat `<tên> …` không khớp vì phần trước hậu tố không phải một tên hợp lệ.
pub fn parse_log_line(line: &str) -> Option<PlayerLogEvent> {
    let line = line.trim_end_matches(['\r', '\n']);
    parse_message(log_message(line))
}

fn log_message(line: &str) -> &str {
    let Some(rest) = line.strip_prefix('[') else {
        return line;
    };
    let Some(end) = rest.find(']') else {
        return line;
    };
    let inside = &rest[..end];
    if !is_stamp(inside) {
        return line;
    }
    let after = &rest[end + 1..];
    vanilla_body(after)
        .or_else(|| paper_body(inside, after))
        .unwrap_or(line)
}

/// `12:34:56` hoặc `12:34:56 INFO`. Không nhận ngoặc khác, kẻo một dòng chat bắt đầu bằng `[`
/// bị cắt nhầm thành sự kiện join.
fn is_stamp(inside: &str) -> bool {
    let time = match inside.split_once(' ') {
        Some((time, level)) => {
            if !is_log_level(level) {
                return false;
            }
            time
        }
        None => inside,
    };
    is_hhmmss(time)
}

fn is_log_level(level: &str) -> bool {
    !level.is_empty() && level.bytes().all(|byte| byte.is_ascii_uppercase())
}

fn is_hhmmss(time: &str) -> bool {
    let mut parts = time.split(':');
    let Some(hour) = parts.next() else {
        return false;
    };
    let Some(minute) = parts.next() else {
        return false;
    };
    let Some(second) = parts.next() else {
        return false;
    };
    parts.next().is_none() && is_two_digits(hour) && is_two_digits(minute) && is_two_digits(second)
}

fn is_two_digits(text: &str) -> bool {
    text.len() == 2 && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// Phần sau mốc giờ của Vanilla: ` [Server thread/INFO]: nội dung`.
fn vanilla_body(after: &str) -> Option<&str> {
    let inner = after.strip_prefix(" [")?;
    let end = inner.find("]: ")?;
    let thread = &inner[..end];
    thread.contains('/').then_some(&inner[end + 3..])
}

/// Console Paper: mức log nằm trong cùng ngoặc với giờ, `[12:34:56 INFO]: nội dung`.
fn paper_body<'a>(inside: &str, after: &'a str) -> Option<&'a str> {
    let (_, level) = inside.split_once(' ')?;
    if !is_log_level(level) {
        return None;
    }
    after.strip_prefix(": ")
}

fn parse_message(message: &str) -> Option<PlayerLogEvent> {
    parse_join_or_leave(message).or_else(|| parse_list(message))
}

fn parse_join_or_leave(message: &str) -> Option<PlayerLogEvent> {
    if let Some(name) = message.strip_suffix(" joined the game") {
        return Some(PlayerLogEvent::Joined(player_token(name)?));
    }
    if let Some(name) = message.strip_suffix(" left the game") {
        return Some(PlayerLogEvent::Left(player_token(name)?));
    }
    None
}

/// Hậu tố đã gồm dấu cách. `<Notch> joined the game` còn lại `<Notch>`, không phải tên hợp lệ.
fn player_token(name: &str) -> Option<PlayerName> {
    PlayerName::parse(name).ok()
}

fn parse_list(message: &str) -> Option<PlayerLogEvent> {
    let rest = message.strip_prefix("There are ")?;
    let (online, rest) = take_u32(rest)?;
    let rest = rest.strip_prefix(" of a max of ")?;
    let (max, rest) = take_u32(rest)?;
    let rest = rest.strip_prefix(" players online:")?;
    // Vanilla để `%s` rỗng nên dòng 0 người thường có một dấu cách sau dấu hai chấm.
    let names = parse_name_list(rest.trim())?;
    Some(PlayerLogEvent::List { online, max, names })
}

fn take_u32(text: &str) -> Option<(u32, &str)> {
    let end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    if end == 0 {
        return None;
    }
    let value: u32 = text[..end].parse().ok()?;
    Some((value, &text[end..]))
}

/// Token không phải tên hợp lệ thì cả dòng bị bỏ. `list uuids` gắn UUID sau tên;
/// cắt nửa danh sách sẽ báo sai ai đang online.
fn parse_name_list(raw: &str) -> Option<Vec<PlayerName>> {
    if raw.is_empty() {
        return Some(Vec::new());
    }
    let mut names = Vec::new();
    for part in raw.split(", ") {
        names.push(PlayerName::parse(part).ok()?);
    }
    Some(names)
}
