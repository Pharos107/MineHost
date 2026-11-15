//! So khớp ràng buộc phiên bản đơn giản trong metadata mod (spec P3, mục 5.1).
//!
//! Dùng để cảnh báo thiếu hoặc lệch phiên bản dependency trước khi bật server, không dùng để chặn
//! cài: mọi dạng ràng buộc chưa hỗ trợ (như `~`, `^`) trả `None` ("không biết") thay vì báo lỗi.

use std::cmp::Ordering;

/// Kiểm tra `version` có thỏa mãn `req` hay không.
///
/// Hỗ trợ:
/// - `*` hoặc chuỗi rỗng (khớp mọi phiên bản không rỗng)
/// - Wildcard ở đoạn cuối (`0.8.x`, `1.21.X`, `1.21.*`) khớp mọi bản cùng tiền tố số
/// - `>=`, `<=`, `>`, `<`, `=` (hoặc phiên bản trần hiểu là `=`), nhiều điều kiện cách nhau bằng
///   khoảng trắng (AND, kiểu Fabric), hoặc các nhánh ngăn cách bằng `||` (OR)
/// - Khoảng Maven: `[1.20.1]`, `[1.20,1.21)`, `[47,)`, `(1.6.15,)`
///
/// Trả `None` nếu `version` hoặc `req` không phân tích được theo các dạng trên.
pub fn matches_version_req(version: &str, req: &str) -> Option<bool> {
    let version = version.trim();
    if version.is_empty() {
        return None;
    }
    let req = req.trim();
    if req.is_empty() || req == "*" {
        return Some(true);
    }

    let ver = parse_version(version)?;
    let mut has_unknown = false;
    for branch in req.split("||") {
        match matches_clause(&ver, branch.trim()) {
            Some(true) => return Some(true),
            Some(false) => {}
            None => has_unknown = true,
        }
    }
    if has_unknown { None } else { Some(false) }
}

fn matches_clause(ver: &ParsedVersion<'_>, clause: &str) -> Option<bool> {
    if clause.is_empty() {
        return None;
    }
    if clause == "*" {
        return Some(true);
    }
    if (clause.starts_with('[') || clause.starts_with('('))
        && (clause.ends_with(']') || clause.ends_with(')'))
    {
        return matches_maven_range(ver, clause);
    }

    let mut all_match = true;
    for token in clause.split_whitespace() {
        if token == "*" {
            continue;
        }
        all_match &= match_comparison_token(ver, token)?;
    }
    Some(all_match)
}

fn match_comparison_token(ver: &ParsedVersion<'_>, token: &str) -> Option<bool> {
    let (op, raw_ver) = if let Some(rest) = token.strip_prefix(">=") {
        (CmpOp::Ge, rest)
    } else if let Some(rest) = token.strip_prefix("<=") {
        (CmpOp::Le, rest)
    } else if let Some(rest) = token.strip_prefix('>') {
        (CmpOp::Gt, rest)
    } else if let Some(rest) = token.strip_prefix('<') {
        (CmpOp::Lt, rest)
    } else if let Some(rest) = token.strip_prefix('=') {
        (CmpOp::Eq, rest)
    } else {
        (CmpOp::Eq, token)
    };
    if matches!(op, CmpOp::Eq)
        && let Some(prefix) = parse_wildcard_prefix(raw_ver)
    {
        return Some(
            prefix
                .iter()
                .enumerate()
                .all(|(i, &p)| ver.nums.get(i).copied().unwrap_or(0) == p),
        );
    }
    let target = parse_version(raw_ver)?;
    let ord = ver.cmp_ver(&target);
    Some(match op {
        CmpOp::Ge => ord != Ordering::Less,
        CmpOp::Le => ord != Ordering::Greater,
        CmpOp::Gt => ord == Ordering::Greater,
        CmpOp::Lt => ord == Ordering::Less,
        CmpOp::Eq => ord == Ordering::Equal,
    })
}

fn parse_wildcard_prefix(raw: &str) -> Option<Vec<u64>> {
    let raw = raw.trim();
    if raw.contains('+') || raw.contains('-') {
        return None;
    }
    let mut segs: Vec<&str> = raw.split('.').collect();
    let last = segs.pop()?;
    if segs.is_empty() || !matches!(last, "x" | "X" | "*") {
        return None;
    }
    segs.into_iter()
        .map(|s| {
            (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                .then(|| s.parse::<u64>().ok())
                .flatten()
        })
        .collect()
}

fn matches_maven_range(ver: &ParsedVersion<'_>, range: &str) -> Option<bool> {
    let open_left = range.starts_with('(');
    let open_right = range.ends_with(')');
    let inner = range.get(1..range.len().saturating_sub(1))?;

    let mut parts = inner.split(',');
    let first = parts.next()?.trim();
    let Some(second) = parts.next() else {
        // Khoảng đơn `[1.20.1]` khớp chính xác; ngoặc tròn trên một giá trị là sai cú pháp.
        if open_left || open_right {
            return None;
        }
        let exact = parse_version(first)?;
        return Some(ver.cmp_ver(&exact) == Ordering::Equal);
    };
    if parts.next().is_some() {
        return None;
    }
    let second = second.trim();
    if (first.is_empty() && !open_left) || (second.is_empty() && !open_right) {
        return None;
    }

    let lower_ok = match first.is_empty() {
        true => true,
        false => {
            let ord = ver.cmp_ver(&parse_version(first)?);
            if open_left {
                ord == Ordering::Greater
            } else {
                ord != Ordering::Less
            }
        }
    };
    let upper_ok = match second.is_empty() {
        true => true,
        false => {
            let ord = ver.cmp_ver(&parse_version(second)?);
            if open_right {
                ord == Ordering::Less
            } else {
                ord != Ordering::Greater
            }
        }
    };
    Some(lower_ok && upper_ok)
}

#[derive(Clone, Copy)]
enum CmpOp {
    Ge,
    Le,
    Gt,
    Lt,
    Eq,
}

struct ParsedVersion<'a> {
    nums: Vec<u64>,
    pre: Option<&'a str>,
}

impl ParsedVersion<'_> {
    fn cmp_ver(&self, other: &Self) -> Ordering {
        let len = self.nums.len().max(other.nums.len());
        for i in 0..len {
            let a = self.nums.get(i).copied().unwrap_or(0);
            let b = other.nums.get(i).copied().unwrap_or(0);
            if a != b {
                return a.cmp(&b);
            }
        }
        match (self.pre, other.pre) {
            (None, None) => Ordering::Equal,
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(a), Some(b)) => cmp_prerelease(a, b),
        }
    }
}

fn parse_version(raw: &str) -> Option<ParsedVersion<'_>> {
    let without_build = raw.trim().split('+').next()?.trim();
    if without_build.is_empty() {
        return None;
    }
    let (core, pre) = match without_build.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (without_build, None),
    };
    if core.is_empty() {
        return None;
    }
    let nums = core
        .split('.')
        .map(|s| {
            (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                .then(|| s.parse::<u64>().ok())
                .flatten()
        })
        .collect::<Option<Vec<_>>>()?;
    Some(ParsedVersion { nums, pre })
}

fn cmp_prerelease(a: &str, b: &str) -> Ordering {
    let mut pa = a.split('.');
    let mut pb = b.split('.');
    loop {
        match (pa.next(), pb.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(sa), Some(sb)) => {
                let ord = match (sa.parse::<u64>(), sb.parse::<u64>()) {
                    (Ok(na), Ok(nb)) => na.cmp(&nb),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => sa.cmp(sb),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_and_simple_comparisons() {
        assert_eq!(matches_version_req("1.21.1", "*"), Some(true));
        assert_eq!(matches_version_req("${file.jarVersion}", "*"), Some(true));
        assert_eq!(matches_version_req("0.8.13+mc1.21.1", "0.8.x"), Some(true));
        assert_eq!(matches_version_req("0.8", "0.8.x"), Some(true));
        assert_eq!(matches_version_req("0.9.0", "0.8.x"), Some(false));
        assert_eq!(matches_version_req("0.80.1", "0.8.x"), Some(false));
        assert_eq!(matches_version_req("1.21.4", "1.21.X"), Some(true));
        assert_eq!(matches_version_req("1.21.4", "1.21.*"), Some(true));
        assert_eq!(matches_version_req("1.22.0", "1.21.*"), Some(false));
        assert_eq!(matches_version_req("1.5.0", "1.x"), Some(true));
        assert_eq!(matches_version_req("2.0.0", "1.x"), Some(false));
        assert_eq!(matches_version_req("0.16.9", ">=0.16.0"), Some(true));
        assert_eq!(matches_version_req("0.15.11", ">=0.16.0"), Some(false));
        assert_eq!(matches_version_req("1.20.0", "=1.20"), Some(true));
        assert_eq!(matches_version_req("1.20.1", ">1.20"), Some(true));
        assert_eq!(matches_version_req("1.20.1", "<=1.20.1"), Some(true));
        assert_eq!(matches_version_req("1.20.2", "<=1.20.1"), Some(false));
    }

    #[test]
    fn fabric_compound_and_or_ranges() {
        assert_eq!(
            matches_version_req("1.21.1", ">=1.21- <1.21.2-"),
            Some(true)
        );
        assert_eq!(
            matches_version_req("1.21.2-alpha.1", ">=1.21- <1.21.2-"),
            Some(false)
        );
        assert_eq!(
            matches_version_req("1.21-beta.2", ">=1.21-beta.2"),
            Some(true)
        );
        assert_eq!(
            matches_version_req("1.21-beta.1", ">=1.21-beta.2"),
            Some(false)
        );
        assert_eq!(matches_version_req("1.21.1", "1.21 || 1.21.1"), Some(true));
        assert_eq!(matches_version_req("1.20.4", "1.21 || 1.21.1"), Some(false));
        assert_eq!(
            matches_version_req("0.8.13+mc1.21.1", "0.8.x || 0.9.0"),
            Some(true)
        );
        assert_eq!(matches_version_req("1.0", "0.8.x || 0.9.0"), Some(false));
    }

    #[test]
    fn maven_ranges() {
        assert_eq!(matches_version_req("1.20.1", "[1.20,1.21)"), Some(true));
        assert_eq!(matches_version_req("1.20", "[1.20,1.21)"), Some(true));
        assert_eq!(matches_version_req("1.21", "[1.20,1.21)"), Some(false));
        assert_eq!(matches_version_req("47.3.0", "[47,)"), Some(true));
        assert_eq!(matches_version_req("46.0.0", "[47,)"), Some(false));
        assert_eq!(matches_version_req("1.6.15", "(1.6.15,)"), Some(false));
        assert_eq!(matches_version_req("1.6.16", "(1.6.15,)"), Some(true));
        assert_eq!(matches_version_req("1.20.1", "[1.20.1]"), Some(true));
        assert_eq!(matches_version_req("1.20.2", "[1.20.1]"), Some(false));
        assert_eq!(matches_version_req("21.1.0", "(,21.1.0]"), Some(true));
        assert_eq!(matches_version_req("21.1.1", "(,21.1.0]"), Some(false));
    }

    #[test]
    fn unknown_patterns_return_none() {
        assert_eq!(matches_version_req("1.20.1", "~1.20"), None);
        assert_eq!(matches_version_req("1.20.1", "^1.20"), None);
        assert_eq!(matches_version_req("1.0.0", "1.x.0"), None);
        assert_eq!(matches_version_req("1.0.0", ">=1.0.x"), None);
        assert_eq!(matches_version_req("${file.jarVersion}", "[1.0,)"), None);
        assert_eq!(matches_version_req("", ">=1.0"), None);
        assert_eq!(matches_version_req("1.0", "[,1.0)"), None);
        assert_eq!(matches_version_req("1.0", "[1.0,]"), None);
        assert_eq!(matches_version_req("1.0", "(1.0)"), None);
        assert_eq!(matches_version_req("1.0", ">=1.0 ~2.0"), None);
        assert_eq!(matches_version_req("1.0", "~0.8.0 || 0.9.0"), None);
    }
}
