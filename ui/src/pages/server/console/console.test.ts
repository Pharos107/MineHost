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

describe("tải log", () => {
  it("tên file an toàn, có ngày giờ; bỏ mã màu", () => {
    expect(logFileName("Prominence II", new Date(2026, 10, 16, 9, 5))).toBe("Prominence-II-console-2026-11-16-0905.txt");
    expect(logFileName("../", new Date(2026, 0, 1, 0, 0))).toBe("server-console-2026-01-01-0000.txt");
    expect(logText(["\u001b[33mvàng\u001b[0m", "thường"])).toBe("vàng\nthường\n");
  });
});
