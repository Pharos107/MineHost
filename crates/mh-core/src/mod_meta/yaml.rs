//! Parser tay cho tập con YAML của `plugin.yml` và `paper-plugin.yml`.
//!
//! Lý do tự viết thay vì thêm crate YAML: MineHost chỉ cần vài trường đầu vào (`name`, `version`,
//! `depend`, `softdepend`, `dependencies.server.<tên>.required`).
//!
//! Giới hạn của parser tay:
//! - Không hỗ trợ anchor/alias (`&a`, `*a`), tag (`!`), merge key (`<<`), hoặc flow mapping lồng nhau.
//! - Không dùng tab để thụt lề; chỉ đọc các khóa đích và bỏ qua các khối con khác (như `commands`,
//!   `permissions`), kể cả block scalar (`|`, `>`) nằm bên trong các khối bị bỏ qua đó.

use super::{
    ModDependency, ModMeta, ModMetaError, ModSide, normalize_version, require_field, strip_bom,
};

/// Đọc `plugin.yml` (Bukkit/Spigot/Paper). Plugin luôn chạy ở phía server.
pub(super) fn parse_plugin_yml(text: &str) -> Result<ModMeta, ModMetaError> {
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut depends = Vec::new();
    let mut list_required: Option<bool> = None;
    let mut skip_top_list = false;

    for (indent, trimmed) in iter_yaml_lines(text)? {
        if indent > 0 {
            if let Some(req) = list_required {
                push_dep(&mut depends, parse_seq_item(trimmed)?, req);
            }
            continue;
        }
        if is_seq_item(trimmed) {
            if let Some(req) = list_required {
                push_dep(&mut depends, parse_seq_item(trimmed)?, req);
            } else if !skip_top_list {
                return Err(stray_seq_err(trimmed));
            }
            continue;
        }
        list_required = None;
        skip_top_list = false;
        let (key, val) = parse_key_val(trimmed)?;
        match key.as_str() {
            "name" => name = Some(unquote(val)?),
            "version" => version = Some(unquote(val)?),
            "depend" | "softdepend" => {
                let req = key == "depend";
                if val.is_empty() {
                    list_required = Some(req);
                } else if val.starts_with('[') {
                    for item in parse_flow_list(val)? {
                        push_dep(&mut depends, item, req);
                    }
                } else if !matches!(val, "~" | "null") {
                    push_dep(&mut depends, unquote(val)?, req);
                }
            }
            _ if val.is_empty() => skip_top_list = true,
            _ => {}
        }
    }
    build_server_meta(name, version, depends)
}

enum PaperSection {
    Top,
    Dependencies {
        child_indent: Option<usize>,
    },
    Server {
        server_indent: usize,
        entry_indent: Option<usize>,
        current_dep: Option<usize>,
    },
}

/// Đọc `paper-plugin.yml` (Paper hiện đại). Plugin luôn chạy ở phía server.
pub(super) fn parse_paper_plugin_yml(text: &str) -> Result<ModMeta, ModMetaError> {
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut depends = Vec::new();
    let mut section = PaperSection::Top;
    let mut skip_top_list = false;

    for (indent, trimmed) in iter_yaml_lines(text)? {
        if indent > 0 {
            handle_paper_nested(&mut section, &mut depends, indent, trimmed)?;
            continue;
        }
        section = PaperSection::Top;
        if is_seq_item(trimmed) {
            if !skip_top_list {
                return Err(stray_seq_err(trimmed));
            }
            continue;
        }
        skip_top_list = false;
        let (key, val) = parse_key_val(trimmed)?;
        match key.as_str() {
            "name" => name = Some(unquote(val)?),
            "version" => version = Some(unquote(val)?),
            "dependencies" if val.is_empty() => {
                section = PaperSection::Dependencies { child_indent: None };
            }
            "dependencies" if !is_empty_yaml_val(val) => {
                return Err(ModMetaError::Yaml(
                    "khóa `dependencies` trong paper-plugin.yml phải là bảng".into(),
                ));
            }
            _ if val.is_empty() => skip_top_list = true,
            _ => {}
        }
    }
    build_server_meta(name, version, depends)
}

fn handle_paper_nested(
    section: &mut PaperSection,
    depends: &mut Vec<ModDependency>,
    indent: usize,
    trimmed: &str,
) -> Result<(), ModMetaError> {
    if let PaperSection::Server { server_indent, .. } = *section
        && indent <= server_indent
    {
        *section = PaperSection::Dependencies {
            child_indent: Some(server_indent),
        };
    }
    match section {
        PaperSection::Top => Ok(()),
        PaperSection::Dependencies { child_indent } => {
            let expected = *child_indent.get_or_insert(indent);
            if indent > expected {
                return Ok(());
            }
            let (key, val) = parse_key_val(trimmed)?;
            if key == "server" {
                if val.is_empty() {
                    *section = PaperSection::Server {
                        server_indent: indent,
                        entry_indent: None,
                        current_dep: None,
                    };
                } else if !is_empty_yaml_val(val) {
                    return Err(ModMetaError::Yaml(
                        "khóa `dependencies.server` phải là bảng".into(),
                    ));
                }
            }
            Ok(())
        }
        PaperSection::Server {
            entry_indent,
            current_dep,
            ..
        } => {
            let expected = *entry_indent.get_or_insert(indent);
            if indent == expected {
                let (dep_id, val) = parse_key_val(trimmed)?;
                if val.starts_with('{') {
                    push_dep(depends, dep_id, parse_inline_required(val)?);
                    *current_dep = None;
                } else if val.is_empty() || is_empty_yaml_val(val) {
                    push_dep(depends, dep_id, true);
                    *current_dep = val.is_empty().then_some(depends.len() - 1);
                } else {
                    return Err(ModMetaError::Yaml(format!(
                        "mục `{dep_id}` dưới `dependencies.server` không hợp lệ: {val}"
                    )));
                }
            } else if let Some(idx) = *current_dep {
                let (prop, val) = parse_key_val(trimmed)?;
                if prop == "required"
                    && let Some(dep) = depends.get_mut(idx)
                {
                    dep.required = parse_yaml_bool(val)?;
                }
            }
            Ok(())
        }
    }
}

fn build_server_meta(
    name: Option<String>,
    version: Option<String>,
    depends: Vec<ModDependency>,
) -> Result<ModMeta, ModMetaError> {
    let id = require_field(name.as_deref(), "id")?;
    Ok(ModMeta {
        name: id.clone(),
        id,
        version: normalize_version(version.as_deref()),
        side: ModSide::Server,
        depends,
    })
}

fn push_dep(depends: &mut Vec<ModDependency>, id: String, required: bool) {
    if !id.is_empty() {
        depends.push(ModDependency {
            id,
            version_req: "*".to_owned(),
            required,
        });
    }
}

fn iter_yaml_lines(text: &str) -> Result<Vec<(usize, &str)>, ModMetaError> {
    let mut lines = Vec::new();
    for raw in strip_bom(text).lines() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let indent = line.bytes().take_while(|&b| b == b' ').count();
        let after = &line[indent..];
        if after.starts_with('\t') {
            return Err(ModMetaError::Yaml(
                "YAML không cho phép dùng ký tự tab để thụt lề".into(),
            ));
        }
        let trimmed = strip_comment(after)?.trim();
        if !trimmed.is_empty() && !(indent == 0 && (trimmed == "---" || trimmed == "...")) {
            lines.push((indent, trimmed));
        }
    }
    Ok(lines)
}

fn stray_seq_err(trimmed: &str) -> ModMetaError {
    ModMetaError::Yaml(format!(
        "phần tử danh sách không nằm dưới khóa nào: {trimmed}"
    ))
}

fn strip_comment(s: &str) -> Result<&str, ModMetaError> {
    let mut quote = QuoteState::default();
    let mut prev_ws = true;
    for (idx, ch) in s.char_indices() {
        if !quote.step(ch) && ch == '#' && prev_ws {
            return Ok(&s[..idx]);
        }
        prev_ws = ch.is_whitespace();
    }
    quote.ensure_closed("chuỗi ngoặc trong YAML chưa đóng")?;
    Ok(s)
}

fn parse_key_val(trimmed: &str) -> Result<(String, &str), ModMetaError> {
    let mut quote = QuoteState::default();
    for (idx, ch) in trimmed.char_indices() {
        if !quote.step(ch) && ch == ':' {
            let rest = &trimmed[idx + 1..];
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                return Ok((unquote(trimmed[..idx].trim())?, rest.trim()));
            }
        }
    }
    Err(ModMetaError::Yaml(format!(
        "dòng YAML không phải cặp `khóa: giá trị`: {trimmed}"
    )))
}

fn unquote(s: &str) -> Result<String, ModMetaError> {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('"').and_then(|t| t.strip_suffix('"'))
        && s.len() >= 2
    {
        return Ok(unescape_double(inner));
    }
    if let Some(inner) = s.strip_prefix('\'').and_then(|t| t.strip_suffix('\''))
        && s.len() >= 2
    {
        return Ok(inner.replace("''", "'"));
    }
    if s.starts_with('"') || s.starts_with('\'') {
        return Err(ModMetaError::Yaml(format!("chuỗi ngoặc chưa đóng: {s}")));
    }
    Ok(s.to_owned())
}

fn unescape_double(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn is_seq_item(trimmed: &str) -> bool {
    trimmed == "-" || trimmed.starts_with("- ")
}

fn parse_seq_item(trimmed: &str) -> Result<String, ModMetaError> {
    trimmed
        .strip_prefix('-')
        .ok_or_else(|| ModMetaError::Yaml(format!("mong đợi `- ...`, gặp: {trimmed}")))
        .and_then(unquote)
}

fn parse_flow_list(val: &str) -> Result<Vec<String>, ModMetaError> {
    let inner = val
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| ModMetaError::Yaml(format!("danh sách `[...]` chưa đóng: {val}")))?;
    split_flow_items(inner)?
        .into_iter()
        .map(unquote)
        .filter(|r| !matches!(r, Ok(s) if s.is_empty()))
        .collect()
}

fn parse_inline_required(val: &str) -> Result<bool, ModMetaError> {
    let inner = val
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .ok_or_else(|| ModMetaError::Yaml(format!("bảng `{{...}}` chưa đóng: {val}")))?;
    for part in split_flow_items(inner)? {
        let (k, v) = parse_key_val(part)?;
        if k == "required" {
            return parse_yaml_bool(v);
        }
    }
    Ok(true)
}

fn split_flow_items(inner: &str) -> Result<Vec<&str>, ModMetaError> {
    let mut items = Vec::new();
    let mut start = 0;
    let mut quote = QuoteState::default();
    for (idx, ch) in inner.char_indices() {
        if !quote.step(ch) && ch == ',' {
            let piece = inner[start..idx].trim();
            if !piece.is_empty() {
                items.push(piece);
            }
            start = idx + 1;
        }
    }
    quote.ensure_closed("chuỗi ngoặc chưa đóng trong danh sách")?;
    let tail = inner[start..].trim();
    if !tail.is_empty() {
        items.push(tail);
    }
    Ok(items)
}

#[derive(Default)]
struct QuoteState {
    in_single: bool,
    in_double: bool,
    escaped: bool,
}

impl QuoteState {
    fn step(&mut self, ch: char) -> bool {
        if self.in_double {
            if self.escaped {
                self.escaped = false;
            } else if ch == '\\' {
                self.escaped = true;
            } else if ch == '"' {
                self.in_double = false;
            }
            return true;
        }
        if self.in_single {
            if ch == '\'' {
                self.in_single = false;
            }
            return true;
        }
        match ch {
            '"' => {
                self.in_double = true;
                true
            }
            '\'' => {
                self.in_single = true;
                true
            }
            _ => false,
        }
    }

    fn ensure_closed(&self, msg: &str) -> Result<(), ModMetaError> {
        if self.in_single || self.in_double {
            return Err(ModMetaError::Yaml(msg.to_owned()));
        }
        Ok(())
    }
}

fn parse_yaml_bool(val: &str) -> Result<bool, ModMetaError> {
    match val.trim().to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ModMetaError::Yaml(format!(
            "giá trị boolean không hợp lệ: {val}"
        ))),
    }
}

fn is_empty_yaml_val(val: &str) -> bool {
    matches!(val, "~" | "null" | "{}")
}

#[cfg(test)]
#[path = "yaml_tests.rs"]
mod tests;
