import { describe, expect, it } from "vitest";

import { blockSlice } from "./blockSlice";

describe("blockSlice", () => {
  it("cùng chuỗi ra cùng hình, chuỗi khác ra hình khác", () => {
    const a = blockSlice("01M78CC3A32WT5PN76T9ZRHJAV", 40, 6);
    expect(blockSlice("01M78CC3A32WT5PN76T9ZRHJAV", 40, 6)).toEqual(a);
    expect(blockSlice("01M78HY3PB5KA84SXN2QPFF0WJ", 40, 6)).not.toEqual(a);
  });

  it("trời ở trên, đất liền một khối ở dưới; hai hàng cuối luôn kín", () => {
    const grid = blockSlice("server", 40, 6);
    for (let x = 0; x < 40; x++) {
      const column = grid.map((row) => row[x]);
      const ground = column.findIndex((cell) => cell !== null);
      expect(ground).toBeGreaterThanOrEqual(0);
      expect(ground).toBeLessThanOrEqual(2);
      expect(column.slice(ground).every((cell) => cell !== null)).toBe(true);
    }
  });

  it("biểu tượng nhỏ là một khối đặc", () => {
    const grid = blockSlice("MineHost", 4, 4, { flat: true });
    expect(grid.flat().every((cell) => cell !== null)).toBe(true);
  });
});
