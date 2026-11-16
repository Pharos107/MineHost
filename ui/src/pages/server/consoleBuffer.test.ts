import { describe, expect, it } from "vitest";

import { appendCapped, matchesFilter, type BufferedLine } from "./consoleBuffer";

const line = (text: string): BufferedLine => ({ text, tone: "normal" });

describe("bộ đệm console", () => {
  it("giữ tối đa `max` dòng, bỏ dòng cũ nhất", () => {
    const lines: BufferedLine[] = [];
    for (const text of ["a", "b", "c", "d"]) appendCapped(lines, line(text), 3);
    expect(lines.map((l) => l.text)).toEqual(["b", "c", "d"]);
  });

  it("lọc không phân biệt hoa thường, ô trống thì khớp hết", () => {
    expect(matchesFilter("[12:00:01 INFO]: Steve joined the game", "steve")).toBe(true);
    expect(matchesFilter("[12:00:01 INFO]: Steve joined the game", "  JOINED ")).toBe(true);
    expect(matchesFilter("Done (3.2s)!", "error")).toBe(false);
    expect(matchesFilter("bất kỳ", "   ")).toBe(true);
  });

  it("bỏ mã màu ANSI khi lọc", () => {
    expect(matchesFilter("\x1b[33mWARN\x1b[0m Can't keep up!", "warn can't")).toBe(true);
    expect(matchesFilter("\x1b[33mWARN", "33m")).toBe(false);
  });

  it("lọc được tiếng Việt", () => {
    expect(matchesFilter("<Nam> Xin chào cả nhà", "chào")).toBe(true);
  });
});

describe("mức của dòng log", () => {
  it("đọc WARN, ERROR theo cả kiểu Paper và kiểu vanilla / Fabric", async () => {
    const { lineLevel, lineTone } = await import("./consoleBuffer");
    expect(lineLevel("[10:20:11 WARN]: Can't keep up!")).toBe("warn");
    expect(lineLevel("[10:20:11] [Server thread/ERROR]: Encountered an unexpected exception")).toBe("error");
    expect(lineLevel("[10:20:11] [main/FATAL]: crash")).toBe("error");
    expect(lineLevel("[10:20:11 INFO]: <Steve> WARN] chỉ là chat")).toBe("info");
    expect(lineLevel("[16Nov2026 10:20:11.123] [main/WARN] [mixin/]: x")).toBe("warn");
    expect(lineLevel("[10:20:11 INFO]: Steve joined the game")).toBe("info");
    expect(lineTone("stdout", "[10:20:11 WARN]: x")).toBe("warning");
    expect(lineTone("stdout", "[10:20:11 ERROR]: x")).toBe("danger");
    expect(lineTone("stderr", "bất kỳ")).toBe("danger");
    expect(lineTone("stdin", "list")).toBe("primary");
  });
});

describe("lọc theo mức", () => {
  it("Cảnh báo gồm cả lỗi; Chat là tin nhắn và người vào / rời", async () => {
    const { matchesLevel } = await import("./consoleBuffer");
    const warn = "[10:20:11 WARN]: Can't keep up!";
    const error = "[10:20:11] [Server thread/ERROR]: boom";
    const chat = "[10:19:40] [Server thread/INFO]: <Steve> xin chào";
    const join = "[10:19:02 INFO]: Steve joined the game";
    const info = "[10:20:30 INFO]: Saving chunks";
    expect([warn, error, chat, join, info].filter((l) => matchesLevel(l, "warn"))).toEqual([warn, error]);
    expect([warn, error, chat, join, info].filter((l) => matchesLevel(l, "error"))).toEqual([error]);
    expect([warn, error, chat, join, info].filter((l) => matchesLevel(l, "chat"))).toEqual([chat, join]);
    expect(matchesLevel(info, "all")).toBe(true);
  });
});
