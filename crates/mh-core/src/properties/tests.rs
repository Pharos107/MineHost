use proptest::prelude::*;

use super::*;

/// Rút gọn từ file Paper 1.21.4 sinh ra, kèm vài khóa có escape.
const SAMPLE: &str = "#Minecraft server properties\r
#Fri Oct 23 10:00:00 ICT 2026\r
enable-rcon=false\r
level-seed=\r
motd=A Minecraft Server\r
resource-pack=https\\://example.com/pack.zip\r
server-port=25565\r
\r
   # comment thụt lề\r
";

#[test]
fn reads_minecraft_file() {
    let p = Properties::parse(SAMPLE);
    assert_eq!(p.get("motd"), Some("A Minecraft Server"));
    assert_eq!(p.get("level-seed"), Some(""));
    assert_eq!(p.get("resource-pack"), Some("https://example.com/pack.zip"));
    assert_eq!(p.get("server-port"), Some("25565"));
    assert_eq!(p.get("#Minecraft"), None);
    assert_eq!(p.entries().count(), 5);
    assert_eq!(p.to_string(), SAMPLE);
}

#[test]
fn set_rewrites_only_that_line() {
    let mut p = Properties::parse(SAMPLE);
    p.set("server-port", "25570");
    let expected = SAMPLE.replace("server-port=25565", "server-port=25570");
    assert_eq!(p.to_string(), expected);

    // Cùng giá trị thì giữ nguyên văn bản gốc, kể cả cách escape cũ.
    p.set("resource-pack", "https://example.com/pack.zip");
    assert_eq!(p.to_string(), expected);
}

#[test]
fn set_new_key_appends_with_same_line_ending() {
    let mut p = Properties::parse(SAMPLE);
    p.set("pvp", "true");
    assert_eq!(p.to_string(), format!("{SAMPLE}pvp=true\r\n"));
}

#[test]
fn append_after_last_line_without_newline() {
    let mut p = Properties::parse("a=1");
    p.set("b", "2");
    assert_eq!(p.to_string(), "a=1\nb=2\n");

    // Dòng cuối kết thúc bằng `\` không được nuốt mất dòng mới.
    let mut p = Properties::parse("a=1\\");
    p.set("b", "2");
    assert_eq!(p.get("a"), Some("1"));
    assert_eq!(Properties::parse(&p.to_string()).get("b"), Some("2"));

    // Proptest tìm ra: `\` cuối file dù đã có xuống dòng vẫn chờ nối dòng tiếp theo.
    let mut p = Properties::parse("x=1\\\n");
    p.set("b", "2");
    let back = Properties::parse(&p.to_string());
    assert_eq!((back.get("x"), back.get("b")), (Some("1"), Some("2")));
}

#[test]
fn non_ascii_is_written_as_unicode_escape() {
    let mut p = Properties::default();
    p.set("motd", "Server của tôi 🎮");
    let text = p.to_string();
    assert!(text.is_ascii(), "{text}");
    assert!(text.contains("\\u1EE7"), "{text}");
    assert_eq!(
        Properties::parse(&text).get("motd"),
        Some("Server của tôi 🎮")
    );
}

#[test]
fn reads_raw_utf8_and_unicode_escapes() {
    let p = Properties::parse("a=của\nb=c\\u1EE7a\nc=\\uD83C\\uDFAE\nd=\\uD83C\ne=\\u12\n");
    assert_eq!(p.get("a"), Some("của"));
    assert_eq!(p.get("b"), Some("của"));
    assert_eq!(p.get("c"), Some("🎮"));
    assert_eq!(p.get("d"), Some("\u{FFFD}"));
    assert_eq!(p.get("e"), Some("u12"));
}

#[test]
fn separators_follow_java_rules() {
    let cases = [
        ("a=b", "a", "b"),
        ("a:b", "a", "b"),
        ("a b", "a", "b"),
        ("a  =  b", "a", "b"),
        ("a = = b", "a", "= b"),
        ("a=b=c", "a", "b=c"),
        ("my\\ key=1", "my key", "1"),
        ("k\\=x=1", "k=x", "1"),
        ("key", "key", ""),
        ("=v", "", "v"),
        ("  indented=1", "indented", "1"),
        ("tail=x  ", "tail", "x  "),
    ];
    for (line, key, value) in cases {
        let p = Properties::parse(line);
        assert_eq!(p.get(key), Some(value), "{line:?}");
    }
}

#[test]
fn continuation_lines_are_joined() {
    let text = "list = a, \\\n       b, \\\r\n  c\nnext=1\n";
    let p = Properties::parse(text);
    assert_eq!(p.get("list"), Some("a, b, c"));
    assert_eq!(p.get("next"), Some("1"));
    assert_eq!(p.to_string(), text);
    // Dấu `\\` chẵn là ký tự thật, không nối dòng.
    let p = Properties::parse("path=C:\\\\\nnext=1\n");
    assert_eq!(p.get("path"), Some("C:\\"));
    assert_eq!(p.get("next"), Some("1"));
}

#[test]
fn comments_with_both_markers() {
    let p = Properties::parse("#a=1\n!b=2\n \t# c=3\nd=4\n");
    assert_eq!(p.entries().collect::<Vec<_>>(), [("d", "4")]);
}

#[test]
fn duplicate_keys_last_wins() {
    let mut p = Properties::parse("a=1\nb=2\na=3\n");
    assert_eq!(p.get("a"), Some("3"));
    assert_eq!(p.entries().collect::<Vec<_>>(), [("b", "2"), ("a", "3")]);
    p.set("a", "4");
    assert_eq!(p.to_string(), "a=1\nb=2\na=4\n");
}

#[test]
fn remove_deletes_every_copy() {
    let mut p = Properties::parse("#x\na=1\nb=2\na=3\n");
    assert!(p.remove("a"));
    assert!(!p.remove("a"));
    assert_eq!(p.to_string(), "#x\nb=2\n");
}

#[test]
fn escapes_special_keys_and_values() {
    let mut p = Properties::default();
    p.set("#not comment", " leading\tand\\back:=!");
    p.set("", "empty key");
    let text = p.to_string();
    let back = Properties::parse(&text);
    assert_eq!(
        back.get("#not comment"),
        Some(" leading\tand\\back:=!"),
        "{text}"
    );
    assert_eq!(back.get(""), Some("empty key"), "{text}");
}

/// Bảng chữ cái nhỏ nhưng có đủ ký tự đặc biệt, để proptest hay gặp trường hợp khó.
const ALPHABET: &str = r"[ab =:#!\\u0Fư\t\r\n\x0c]{0,80}";

proptest! {
    #[test]
    fn parse_then_write_is_identity(text in ALPHABET) {
        prop_assert_eq!(Properties::parse(&text).to_string(), text);
    }

    #[test]
    fn set_then_parse_returns_value(text in ALPHABET, key in ALPHABET, value in ALPHABET) {
        let mut p = Properties::parse(&text);
        let others: Vec<(String, String)> = p
            .entries()
            .filter(|(k, _)| *k != key)
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect();
        p.set(&key, &value);
        let back = Properties::parse(&p.to_string());
        prop_assert_eq!(back.get(&key), Some(value.as_str()));
        for (k, v) in &others {
            prop_assert_eq!(back.get(k), Some(v.as_str()));
        }
    }
}
