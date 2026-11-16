use super::*;

/// Mẫu thật: `help` trên Prominence II (Fabric 1.20.1), kèm vài dòng không phải lệnh.
const FABRIC: &str = include_str!("../../tests/fixtures/help/fabric-prominence.txt");
/// Mẫu thật: Paper 1.20.4, `minecraft:help` rồi `help` (trang Bukkit).
const PAPER: &str = include_str!("../../tests/fixtures/help/paper.txt");

fn commands(sample: &str) -> Vec<ServerCommand> {
    merge_help(sample.lines().filter_map(parse_help_line))
}

fn find<'a>(list: &'a [ServerCommand], name: &str) -> &'a ServerCommand {
    list.iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("thiếu lệnh {name}"))
}

#[test]
fn doc_mau_fabric_gom_ca_lenh_cua_mod() {
    let list = commands(FABRIC);
    assert_eq!(list.len(), 12);
    assert_eq!(
        find(&list, "gamemode").usage.as_deref(),
        Some("gamemode <gamemode> [<target>]")
    );
    assert_eq!(
        find(&list, "ftbquests").usage.as_deref(),
        Some("ftbquests open_book [<target>]")
    );
    assert!(list.iter().any(|c| c.name == "prominent:hourglass"));
    assert!(list.iter().any(|c| c.name == "minecells:tp"));
    assert_eq!(find(&list, "tp").usage.as_deref(), Some("tp -> teleport"));
}

#[test]
fn bo_dong_khong_phai_lenh() {
    let list = commands(FABRIC);
    // `[STDOUT]: /home/...`, chat có `/tp`, `> help`, dòng lỗi netty.
    assert!(list.iter().all(|c| !c.name.starts_with("home")));
    assert_eq!(
        parse_help_line("[05:34:41] [Server thread/INFO]: <Steve> /tp a"),
        None
    );
    assert_eq!(parse_help_line("> help"), None);
    assert_eq!(parse_help_line("/home/danglh/server"), None);
    assert_eq!(parse_help_line("[05:34:47] [Server thread/INFO]: /"), None);
}

#[test]
fn cat_cach_go_qua_dai() {
    let list = commands(FABRIC);
    let usage = find(&list, "gamerule").usage.clone().unwrap_or_default();
    assert_eq!(usage.chars().count(), MAX_USAGE_CHARS + 1);
    assert!(usage.ends_with('…'));
}

#[test]
fn doc_mau_paper_gop_cach_go_va_mo_ta() {
    let list = commands(PAPER);
    let advancement = find(&list, "advancement");
    assert_eq!(
        advancement.usage.as_deref(),
        Some("advancement (grant|revoke)")
    );
    assert_eq!(advancement.description, None);
    let mspt = find(&list, "mspt");
    assert_eq!(mspt.usage, None);
    assert_eq!(mspt.description.as_deref(), Some("View server tick times"));
    // "A Mojang provided command." không phải mô tả có ích.
    assert_eq!(find(&list, "say").description, None);
    assert!(list.iter().any(|c| c.name == "minecraft:help"));
    assert!(list.iter().all(|c| c.name != "Aliases"));
}

#[test]
fn gop_theo_ten_khong_phan_biet_hoa_thuong() {
    let lines = [
        HelpLine::Described {
            name: "Spark".into(),
            description: "Profiler".into(),
        },
        HelpLine::Usage {
            name: "spark".into(),
            usage: "spark [<args>]".into(),
        },
    ];
    let list = merge_help(lines);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].usage.as_deref(), Some("spark [<args>]"));
    assert_eq!(list[0].description.as_deref(), Some("Profiler"));
}

#[test]
fn gioi_han_so_lenh() {
    let lines = (0..MAX_COMMANDS + 5).map(|i| HelpLine::Usage {
        name: format!("cmd{i:05}"),
        usage: format!("cmd{i:05}"),
    });
    assert_eq!(merge_help(lines).len(), MAX_COMMANDS);
}

#[test]
fn lenh_gui_theo_loader() {
    assert_eq!(help_requests(LoaderKind::Fabric), ["help"]);
    assert_eq!(
        help_requests(LoaderKind::Purpur),
        ["minecraft:help", "help"]
    );
}
