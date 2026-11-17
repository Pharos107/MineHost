//! Danh sách lệnh của server, đọc từ output của `help` (spec `lenh-cua-server.md`).
//!
//! Hàm thuần: app gửi lệnh ở [`help_requests`], đưa từng dòng stdout vào [`parse_help_line`] rồi
//! gộp bằng [`merge_help`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::LoaderKind;
use crate::players::log_message;

/// Giới hạn để một modpack lạ không làm `commands.json` phình to.
pub const MAX_COMMANDS: usize = 2000;
/// Có lệnh in cách gõ dài hàng nghìn ký tự (Prominence II: 4.113); gợi ý chỉ cần phần đầu.
pub const MAX_USAGE_CHARS: usize = 300;
const MAX_NAME_LEN: usize = 64;
/// Trang trợ giúp của Bukkit ghi câu này cho mọi lệnh vanilla, không có ích cho người dùng.
const MOJANG_PLACEHOLDER: &str = "A Mojang provided command.";

/// Một lệnh của server. `usage` không có dấu `/` ở đầu, ví dụ `tp <targets> <location>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerCommand {
    pub name: String,
    pub usage: Option<String>,
    pub description: Option<String>,
}

/// Một dòng của `help`: kiểu vanilla `/<tên> <cách gõ>` hoặc kiểu Bukkit `/<tên>: <mô tả>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelpLine {
    Usage { name: String, usage: String },
    Described { name: String, description: String },
}

/// Lệnh cần gửi. Paper, Purpur: `help` là trang của Bukkit (có lệnh plugin, không có cách gõ) nên
/// gửi thêm `minecraft:help` để có cách gõ của lệnh vanilla.
pub fn help_requests(loader: LoaderKind) -> &'static [&'static str] {
    match loader {
        LoaderKind::Paper | LoaderKind::Purpur => &["minecraft:help", "help"],
        _ => &["help"],
    }
}

/// Đọc một dòng stdout; `None` nếu không phải dòng liệt kê lệnh.
pub fn parse_help_line(line: &str) -> Option<HelpLine> {
    let body = log_message(line.trim_end_matches(['\r', '\n'])).trim_end();
    let rest = body.strip_prefix('/')?;
    let (token, tail) = match rest.split_once(' ') {
        Some((token, tail)) => (token, Some(tail)),
        None => (rest, None),
    };
    if let Some(name) = token.strip_suffix(':') {
        let name = valid_name(name)?;
        let description = tail.unwrap_or("").trim().to_owned();
        return Some(HelpLine::Described { name, description });
    }
    let name = valid_name(token)?;
    Some(HelpLine::Usage {
        name,
        usage: truncate(rest),
    })
}

/// Gộp theo tên (không phân biệt hoa thường), xếp theo tên. Tối đa [`MAX_COMMANDS`] lệnh.
pub fn merge_help(lines: impl IntoIterator<Item = HelpLine>) -> Vec<ServerCommand> {
    let mut by_name: BTreeMap<String, ServerCommand> = BTreeMap::new();
    for line in lines {
        let name = match &line {
            HelpLine::Usage { name, .. } | HelpLine::Described { name, .. } => name.clone(),
        };
        let key = name.to_ascii_lowercase();
        if !by_name.contains_key(&key) && by_name.len() >= MAX_COMMANDS {
            continue;
        }
        let entry = by_name.entry(key).or_insert_with(|| ServerCommand {
            name,
            usage: None,
            description: None,
        });
        match line {
            HelpLine::Usage { usage, .. } => {
                entry.usage.get_or_insert(usage);
            }
            HelpLine::Described { description, .. } => {
                if !description.is_empty() && description != MOJANG_PLACEHOLDER {
                    entry.description.get_or_insert(description);
                }
            }
        }
    }
    by_name.into_values().collect()
}

/// Tên lệnh: chữ, số, `_ . : -`, bắt đầu bằng chữ hoặc số. Loại được dòng như `/home/user/...`.
fn valid_name(name: &str) -> Option<String> {
    let first = name.chars().next()?;
    let ok = name.len() <= MAX_NAME_LEN
        && first.is_ascii_alphanumeric()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'));
    ok.then(|| name.to_owned())
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= MAX_USAGE_CHARS {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(MAX_USAGE_CHARS).collect();
    cut.push('…');
    cut
}

pub mod registry;

pub use registry::{RegistryBuilder, RegistryEntry, ServerRegistry, is_lang_asset_path};

#[cfg(test)]
mod tests;
