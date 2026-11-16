import { describe, expect, it } from "vitest";

import { matchCommands } from "./commandHints";
import { logFileName, logText } from "./downloadLog";

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

describe("tải log", () => {
  it("tên file an toàn, có ngày giờ; bỏ mã màu", () => {
    expect(logFileName("Prominence II", new Date(2026, 10, 16, 9, 5))).toBe("Prominence-II-console-2026-11-16-0905.txt");
    expect(logFileName("../", new Date(2026, 0, 1, 0, 0))).toBe("server-console-2026-01-01-0000.txt");
    expect(logText(["\u001b[33mvàng\u001b[0m", "thường"])).toBe("vàng\nthường\n");
  });
});
