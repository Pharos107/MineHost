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
