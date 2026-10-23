//! Đọc ghi `server.properties` theo định dạng của `java.util.Properties`.
//!
//! Mỗi dòng giữ nguyên văn bản gốc, nên đọc rồi ghi lại không đổi một byte nào: comment,
//! dòng trống, thứ tự khóa và kiểu xuống dòng của người dùng đều còn. Chỉ dòng nào bị
//! `set` mới được viết lại.

use std::fmt::{self, Write as _};

/// Khoảng trắng theo `java.util.Properties`: dấu cách, tab, form feed.
const WS: [char; 3] = [' ', '\t', '\x0c'];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Properties {
    lines: Vec<Line>,
}

/// Một dòng logic: có thể gồm nhiều dòng vật lý nối bằng `\` ở cuối.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Line {
    /// Văn bản gốc, kể cả ký tự xuống dòng.
    raw: String,
    /// `None` với comment và dòng trống.
    entry: Option<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    key: String,
    value: String,
}

impl Line {
    fn key(&self) -> Option<&str> {
        self.entry.as_ref().map(|e| e.key.as_str())
    }
}

impl Properties {
    /// Không bao giờ lỗi: giống Java, dòng lạ vẫn được hiểu theo một cách nào đó.
    pub fn parse(text: &str) -> Self {
        let mut lines = Vec::new();
        let mut physical = text.split_inclusive('\n');
        while let Some(first) = physical.next() {
            let mut raw = first.to_owned();
            let mut part = strip_eol(first).trim_start_matches(WS);
            if part.is_empty() || part.starts_with(['#', '!']) {
                lines.push(Line { raw, entry: None });
                continue;
            }
            let mut logical = String::new();
            loop {
                let Some(head) = continuation(part) else {
                    logical.push_str(part);
                    break;
                };
                logical.push_str(head);
                let Some(next) = physical.next() else { break };
                raw.push_str(next);
                part = strip_eol(next).trim_start_matches(WS);
            }
            let (key, value) = split_entry(&logical);
            let entry = Entry {
                key: unescape(key),
                value: unescape(value),
            };
            lines.push(Line {
                raw,
                entry: Some(entry),
            });
        }
        Self { lines }
    }

    /// Giá trị của khóa. Khóa lặp thì lấy dòng cuối, giống Java.
    pub fn get(&self, key: &str) -> Option<&str> {
        let line = self.lines.iter().rev().find(|l| l.key() == Some(key))?;
        line.entry.as_ref().map(|e| e.value.as_str())
    }

    /// Mỗi khóa một lần, theo thứ tự trong file, với giá trị Java thật sự dùng.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.lines.iter().enumerate().filter_map(|(i, line)| {
            let e = line.entry.as_ref()?;
            let later = self
                .lines
                .get(i + 1..)?
                .iter()
                .any(|l| l.key() == Some(&e.key));
            (!later).then_some((e.key.as_str(), e.value.as_str()))
        })
    }

    /// Sửa dòng cuối có khóa này, hoặc thêm dòng mới ở cuối file.
    pub fn set(&mut self, key: &str, value: &str) {
        let eol = self.preferred_eol();
        if let Some(line) = self.lines.iter_mut().rev().find(|l| l.key() == Some(key)) {
            if line.entry.as_ref().is_some_and(|e| e.value == value) {
                return;
            }
            let old_eol = line_ending(&line.raw);
            *line = new_line(key, value, old_eol);
            return;
        }
        if let Some(last) = self.lines.last_mut() {
            close_last_line(last, eol);
        }
        self.lines.push(new_line(key, value, eol));
    }

    /// Xóa mọi dòng có khóa này. Trả `true` nếu có dòng bị xóa.
    pub fn remove(&mut self, key: &str) -> bool {
        let before = self.lines.len();
        self.lines.retain(|l| l.key() != Some(key));
        self.lines.len() != before
    }

    fn preferred_eol(&self) -> &'static str {
        match self.lines.first() {
            Some(l) if l.raw.ends_with("\r\n") => "\r\n",
            _ => "\n",
        }
    }
}

impl fmt::Display for Properties {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.lines.iter().try_for_each(|l| f.write_str(&l.raw))
    }
}

/// Đảm bảo dòng cuối không nuốt dòng sắp thêm vào: phải có ký tự xuống dòng, và không
/// kết thúc bằng `\` nối dòng (file cắt cụt giữa chừng). Dòng như vậy được viết lại.
fn close_last_line(last: &mut Line, eol: &str) {
    let final_physical = strip_eol(&last.raw).rsplit('\n').next().unwrap_or_default();
    let open = continuation(final_physical.trim_start_matches(WS)).is_some();
    *last = match &last.entry {
        Some(e) if open || !last.raw.ends_with('\n') => new_line(&e.key, &e.value, eol),
        None if !last.raw.ends_with('\n') => Line {
            raw: format!("{}{eol}", last.raw),
            entry: None,
        },
        _ => return,
    };
}

fn new_line(key: &str, value: &str, eol: &str) -> Line {
    let mut raw = String::new();
    escape_into(&mut raw, key, true);
    raw.push('=');
    escape_into(&mut raw, value, false);
    raw.push_str(eol);
    let entry = Entry {
        key: key.to_owned(),
        value: value.to_owned(),
    };
    Line {
        raw,
        entry: Some(entry),
    }
}

/// Chỉ coi `\n` và `\r\n` là xuống dòng. Java nhận cả `\r` đứng một mình (kiểu Mac cũ),
/// nhưng Minecraft không bao giờ ghi như vậy.
fn strip_eol(s: &str) -> &str {
    // `\r` cuối file mà không có `\n` theo sau là ký tự thật, nếu không thì thêm dòng mới
    // vào sau sẽ làm nghĩa của dòng này đổi đi.
    match s.strip_suffix('\n') {
        Some(s) => s.strip_suffix('\r').unwrap_or(s),
        None => s,
    }
}

fn line_ending(raw: &str) -> &'static str {
    if raw.ends_with("\r\n") {
        "\r\n"
    } else if raw.ends_with('\n') {
        "\n"
    } else {
        ""
    }
}

/// Phần trước dấu `\` nối dòng, nếu dòng kết thúc bằng một số lẻ dấu `\`.
fn continuation(part: &str) -> Option<&str> {
    let trailing = part.len() - part.trim_end_matches('\\').len();
    (trailing % 2 == 1).then(|| &part[..part.len() - 1])
}

/// Tách khóa và giá trị (chưa bỏ escape) theo quy tắc của `Properties.load`.
fn split_entry(line: &str) -> (&str, &str) {
    let bytes = line.as_bytes();
    let mut key_end = 0;
    while let Some(&b) = bytes.get(key_end) {
        match b {
            b'\\' => key_end += 2,
            b'=' | b':' | b' ' | b'\t' | 0x0c => break,
            _ => key_end += 1,
        }
    }
    // `\` ở cuối có thể đẩy chỉ số vượt độ dài; mọi điểm dừng khác là ký tự ASCII.
    let key_end = key_end.min(bytes.len());
    let mut value_start = key_end;
    let mut has_separator = false;
    while let Some(&b) = bytes.get(value_start) {
        match b {
            b' ' | b'\t' | 0x0c => value_start += 1,
            b'=' | b':' if !has_separator => {
                has_separator = true;
                value_start += 1;
            }
            _ => break,
        }
    }
    (&line[..key_end], &line[value_start..])
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    // Gom các `\uXXXX` liền nhau để ghép đúng cặp surrogate của UTF-16.
    let mut units: Vec<u16> = Vec::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            flush_utf16(&mut units, &mut out);
            out.push(c);
            continue;
        }
        let Some(escaped) = chars.next() else { break };
        if escaped == 'u' {
            let rest = chars.as_str();
            let unit = rest.get(..4).and_then(|h| u16::from_str_radix(h, 16).ok());
            if let (Some(unit), Some(after)) = (unit, rest.get(4..)) {
                units.push(unit);
                chars = after.chars();
                continue;
            }
        }
        flush_utf16(&mut units, &mut out);
        out.push(match escaped {
            't' => '\t',
            'n' => '\n',
            'r' => '\r',
            'f' => '\x0c',
            // Java báo lỗi với `\u` sai; ở đây giữ lại chữ `u` để không mất dữ liệu.
            other => other,
        });
    }
    flush_utf16(&mut units, &mut out);
    out
}

fn flush_utf16(units: &mut Vec<u16>, out: &mut String) {
    let decoded = char::decode_utf16(units.drain(..));
    out.extend(decoded.map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER)));
}

/// Escape như `Properties.store`, thêm `\uXXXX` cho mọi ký tự ngoài ASCII.
///
/// Minecraft cũ đọc file theo ISO-8859-1, bản mới đọc UTF-8; `\uXXXX` đúng với cả hai,
/// nên `motd` tiếng Việt không bị vỡ.
fn escape_into(out: &mut String, s: &str, is_key: bool) {
    for (i, c) in s.chars().enumerate() {
        match c {
            ' ' if is_key || i == 0 => out.push_str("\\ "),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\x0c' => out.push_str("\\f"),
            '=' | ':' | '#' | '!' => {
                out.push('\\');
                out.push(c);
            }
            c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
            c => {
                for unit in c.encode_utf16(&mut [0; 2]) {
                    // Ghi vào String không bao giờ lỗi.
                    let _ = write!(out, "\\u{unit:04X}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
