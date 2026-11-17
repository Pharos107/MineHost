import { describe, expect, it } from "vitest";

import { analyzeConsole, matchArgument, matchCommands } from "./commandHints";
import { parseChoiceGroup } from "./commandTree";
import { logFileName, logText } from "./downloadLog";
import { buildRegistryIndex } from "./registrySearch";

describe("gợi ý lệnh", () => {
  it("chỉ gợi ý khi đang gõ tên lệnh, bỏ qua dấu / ở đầu", () => {
    expect(matchCommands("gam").map((c) => c.name)).toEqual(["gamemode", "gamerule"]);
    expect(matchCommands("/WEA").map((c) => c.name)).toEqual(["weather"]);
    expect(matchCommands("")).toEqual([]);
    expect(matchCommands("gamemode creative")).toEqual([]);
    expect(matchCommands("s").length).toBeLessThanOrEqual(6);
  });
});

describe("gộp lệnh quét từ server", () => {
  const scanned = [
    { name: "ftbquests", usage: "ftbquests open_book [<target>]", description: null },
    { name: "prominent:hourglass", usage: "prominent:hourglass (level|boss)", description: null },
    { name: "tp", usage: "tp -> teleport", description: null },
  ];

  it("thêm lệnh của mod, khớp cả phần sau dấu :, không lặp lệnh có sẵn", () => {
    expect(matchCommands("ftb", scanned).map((c) => c.name)).toEqual(["ftbquests"]);
    expect(matchCommands("hour", scanned).map((c) => c.name)).toEqual(["prominent:hourglass"]);
    const tp = matchCommands("tp", scanned);
    expect(tp.map((c) => c.name)).toEqual(["tp"]);
    expect(tp[0]?.key).toBe("tp");
  });
});

describe("gợi ý đối số", () => {
  const players = [
    { name: "Steve", online: false },
    { name: "Stella", online: true },
    { name: "Alex", online: true },
  ];

  it("tên người chơi khớp chữ đang gõ, người online trước; thay đúng đối số cuối", () => {
    const match = matchArgument("tp Alex st", players);
    expect(match.start).toBe(8);
    expect(match.hints).toEqual([
      { value: "Stella", kind: "online" },
      { value: "Steve", kind: "offline" },
    ]);
  });

  it("bộ chọn @ và không gợi ý khi chưa gõ chữ nào hoặc đang gõ tên lệnh", () => {
    expect(matchArgument("kill @", players).hints.map((h) => h.value)).toEqual(["@a", "@p", "@r", "@s", "@e"]);
    expect(matchArgument("give @p", players).hints).toEqual([{ value: "@p", kind: "selector" }]);
    expect(matchArgument("say ", players).hints).toEqual([]);
    expect(matchArgument("tp", players)).toEqual({ start: 0, hints: [] });
  });
});

describe("tải log", () => {
  it("tải log: tên file an toàn, có ngày giờ; bỏ mã màu", () => {
    expect(logFileName("Prominence II", new Date(2026, 10, 16, 9, 5))).toBe("Prominence-II-console-2026-11-16-0905.txt");
    expect(logFileName("../", new Date(2026, 0, 1, 0, 0))).toBe("server-console-2026-01-01-0000.txt");
    expect(logText(["\u001b[33mvàng\u001b[0m", "thường"])).toBe("vàng\nthường\n");
  });
});

describe("cây lệnh và gợi ý thông minh", () => {
  const players = [
    { name: "Steve", online: false },
    { name: "Stella", online: true },
  ];
  const scanned = [
    { name: "ftbquests", usage: "ftbquests open_book [<target>]", description: null },
    { name: "prominent:hourglass", usage: "prominent:hourglass (level|boss|affix)", description: null },
    { name: "tp", usage: "tp -> teleport", description: null },
    { name: "gamerule", usage: "gamerule (accessories.keepAccessoryInventory|announceAdvancements|cut_off…", description: null },
  ];

  it("gõ / mở danh sách lệnh và giữ nguyên dấu / khi điền", () => {
    const slashOnly = analyzeConsole("/", scanned, players);
    expect(slashOnly.stage).toBe("command");
    expect(slashOnly.hints.length).toBeGreaterThan(0);
    expect(slashOnly.fill("gamemode")).toBe("/gamemode ");

    const withLead = analyzeConsole("  /eff", scanned, players);
    expect(withLead.fill("effect")).toBe("  /effect ");
  });

  it("gamemode gợi ý chế độ chơi ngay sau dấu cách rồi mới gợi ý người chơi", () => {
    const step1 = analyzeConsole("/gamemode ", scanned, players);
    expect(step1.hints.map((h) => (h.group === "option" ? h.value : ""))).toEqual([
      "survival",
      "creative",
      "adventure",
      "spectator",
    ]);
    expect(step1.syntax?.nodes.find((n) => n.active)?.param).toBe("<survival|creative|adventure|spectator>");

    const step2 = analyzeConsole("/gamemode c", scanned, players);
    expect(step2.hints.map((h) => (h.group === "option" ? h.value : ""))).toEqual(["creative"]);
    expect(step2.fill("creative")).toBe("/gamemode creative ");

    const step3 = analyzeConsole("/gamemode creative ", scanned, players);
    expect(step3.hints.some((h) => h.group === "player" && h.value === "Stella")).toBe(true);
    expect(step3.hints.some((h) => h.group === "player" && h.value === "@a")).toBe(true);
    expect(step3.syntax?.nodes.find((n) => n.active)?.paramKey).toBe("playerOpt");
  });

  it("effect đi theo nhánh give/clear và gợi ý đúng vị trí hiệu ứng, giây", () => {
    const sub = analyzeConsole("/effect ", scanned, players);
    expect(sub.hints.map((h) => (h.group === "option" ? h.value : ""))).toEqual(["give", "clear"]);
    expect(sub.fill("give")).toBe("/effect give ");

    const playerStep = analyzeConsole("/effect give ", scanned, players);
    expect(playerStep.hints.some((h) => h.group === "player" && h.value === "Stella")).toBe(true);

    const effectStep = analyzeConsole("/effect give Stella ", scanned, players);
    expect(effectStep.hints.some((h) => h.group === "registry" && h.id === "minecraft:regeneration")).toBe(true);
    expect(effectStep.syntax?.nodes.find((n) => n.active)?.paramKey).toBe("effect");

    const secondsStep = analyzeConsole("/effect give Stella minecraft:regeneration ", scanned, players);
    expect(secondsStep.hints.map((h) => (h.group === "option" ? h.value : ""))).toEqual(["30", "60", "300", "infinite"]);
  });

  it("say không bật danh sách người chơi, chỉ hiện thanh cú pháp", () => {
    const res = analyzeConsole("/say ", scanned, players);
    expect(res.hints).toEqual([]);
    expect(res.syntax?.nodes.find((n) => n.active)?.paramKey).toBe("message");
  });

  it("tách nhánh (a|b|c) từ lệnh mod đã quét và bỏ mảnh cuối bị cắt bởi …", () => {
    expect(parseChoiceGroup("(a|b|cut_off…")).toEqual(["a", "b"]);

    const modCmd = analyzeConsole("/prominent:hourglass ", scanned, players);
    expect(modCmd.hints.map((h) => (h.group === "option" ? h.value : ""))).toEqual(["level", "boss", "affix"]);

    const questStep1 = analyzeConsole("/ftbquests ", scanned, players);
    expect(questStep1.hints.map((h) => (h.group === "option" ? h.value : ""))).toEqual(["open_book"]);

    const questStep2 = analyzeConsole("/ftbquests open_book ", scanned, players);
    expect(questStep2.hints.some((h) => h.group === "player" && h.value === "Stella")).toBe(true);

    const ruleStep = analyzeConsole("/gamerule accessories", scanned, players);
    expect(ruleStep.hints.map((h) => (h.group === "option" ? h.value : ""))).toEqual(["accessories.keepAccessoryInventory"]);
  });

  it("gợi ý hiệu ứng, vật phẩm, sinh vật, phù phép song ngữ Việt–Anh và đồ từ mod", () => {
    const registry = buildRegistryIndex(
      {
        scanned_at: "2026-11-17T00:00:00Z",
        items: [{ id: "create:brass_ingot", label: "Brass Ingot" }],
        effects: [{ id: "irons_spellbooks:hastened", label: "Hastened" }],
        entities: [{ id: "alexsmobs:grizzly_bear", label: "Grizzly Bear" }],
        enchantments: [{ id: "ars_nouveau:mana_boost", label: "Mana Boost" }],
      },
      "vi",
    );

    const regenVi = analyzeConsole("/effect give Stella hoi phuc", scanned, players, registry);
    expect(regenVi.hints[0]).toMatchObject({ group: "registry", id: "minecraft:regeneration", label: "Hồi phục", isMod: false });

    const modEffect = analyzeConsole("/effect give Stella hast", scanned, players, registry);
    expect(modEffect.hints.some((h) => h.group === "registry" && h.id === "irons_spellbooks:hastened" && h.isMod)).toBe(true);

    const diamondVi = analyzeConsole("/give Stella kim cương", scanned, players, registry);
    expect(diamondVi.hints[0]).toMatchObject({ group: "registry", id: "minecraft:diamond", label: "Kim cương" });

    const modItem = analyzeConsole("/give Stella brass", scanned, players, registry);
    expect(modItem.hints[0]).toMatchObject({ group: "registry", id: "create:brass_ingot", isMod: true });

    const summon = analyzeConsole("/summon grizzly", scanned, players, registry);
    expect(summon.hints[0]).toMatchObject({ group: "registry", id: "alexsmobs:grizzly_bear", isMod: true });

    const enchant = analyzeConsole("/enchant Stella mend", scanned, players, registry);
    expect(enchant.hints[0]).toMatchObject({ group: "registry", id: "minecraft:mending" });

    const locate = analyzeConsole("/locate structure ancient", scanned, players, registry);
    expect(locate.hints[0]).toMatchObject({ group: "registry", id: "minecraft:ancient_city" });

    // Gõ prefix `minecraft:` không được nhầm sang item của mod khác.
    const nsOnly = analyzeConsole("/give Stella minecraft:brass", scanned, players, registry);
    expect(nsOnly.hints).toEqual([]);

    // Từ đơn không nằm trong danh sách có sẵn kết thúc bằng dấu cách vẫn chuyển sang tham số kế tiếp.
    const unknownItemNext = analyzeConsole("/give Stella diorite ", scanned, players, registry);
    expect(unknownItemNext.syntax?.nodes.find((n) => n.active)?.paramKey).toBe("countOpt");

    // Tiền tố `minecraft:` vẫn giữ mô tả lệnh gốc trên thanh cú pháp.
    const namespacedCmd = analyzeConsole("/minecraft:effect ", scanned, players, registry);
    expect(namespacedCmd.syntax?.descriptionKey).toBe("console.hints.effect.desc");
  });
});
