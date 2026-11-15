//! Sửa JSON metadata mà Fabric loader vẫn đọc được nhưng `serde_json` từ chối.
//!
//! Nhiều mod (Better End, Testosterone...) để xuống dòng thô trong chuỗi `description`. Fabric
//! loader chấp nhận, nên MineHost cũng phải đọc được, nếu không id của mod bị mất và mod phụ thuộc
//! vào nó bị báo thiếu sai.

/// Thay ký tự điều khiển thô nằm trong chuỗi JSON bằng dạng escape. Ký tự ngoài chuỗi giữ nguyên.
pub(super) fn escape_raw_control_chars(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut in_string = false;
    let mut escaped = false;
    for ch in text.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            } else if (ch as u32) < 0x20 {
                match ch {
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    _ => out.push_str(&format!("\\u{:04x}", ch as u32)),
                }
                continue;
            }
        } else if ch == '"' {
            in_string = true;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::escape_raw_control_chars;

    #[test]
    fn escapes_only_inside_strings() {
        let raw = "{\n  \"a\": \"line1\nline2\t\\\"q\\\" \u{1}\",\n  \"b\": 1\n}";
        let fixed = escape_raw_control_chars(raw);
        assert_eq!(
            fixed,
            "{\n  \"a\": \"line1\\nline2\\t\\\"q\\\" \\u0001\",\n  \"b\": 1\n}"
        );
        let v: serde_json::Value = serde_json::from_str(&fixed).unwrap_or_default();
        assert_eq!(v["a"], "line1\nline2\t\"q\" \u{1}");
    }
}
