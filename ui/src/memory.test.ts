import { describe, expect, it } from "vitest";

import { exceedsAvailable, memoryFor, memoryRange } from "./memory";

const GB = 1024 ** 3;

describe("memoryRange", () => {
  it.each([
    [4, { min: 1024, max: 1024 }, { min: 1024, max: 2048 }],
    [8, { min: 1024, max: 2048 }, { min: 1024, max: 6144 }],
    [16, { min: 1024, max: 4096 }, { min: 1024, max: 14336 }],
    [32, { min: 1024, max: 8192 }, { min: 1024, max: 30720 }],
    [64, { min: 1024, max: 8192 }, { min: 1024, max: 63488 }],
  ])("máy %i GB cho chọn tới total - 2048 MB", (gb, suggested, expected) => {
    expect(memoryRange(gb * GB, suggested)).toEqual(expected);
  });

  it("làm tròn xuống bội số 512 khi RAM máy lẻ", () => {
    // 15,7 GB như máy có card đồ họa tích hợp dùng chung RAM.
    expect(memoryRange(16_100 * 1024 * 1024, { min: 1024, max: 4096 }).max).toBe(13824);
  });

  it("máy quá ít RAM vẫn cho chọn tới mức đề xuất", () => {
    expect(memoryRange(2 * GB, { min: 512, max: 512 })).toEqual({ min: 512, max: 512 });
  });
});

describe("memoryFor", () => {
  it("min là 1024 hoặc bằng max nếu max nhỏ hơn", () => {
    expect(memoryFor(4096)).toEqual({ min: 1024, max: 4096 });
    expect(memoryFor(512)).toEqual({ min: 512, max: 512 });
  });
});

describe("exceedsAvailable", () => {
  it("cảnh báo khi chọn nhiều hơn RAM đang trống", () => {
    expect(exceedsAvailable(4096, 4 * GB)).toBe(false);
    expect(exceedsAvailable(4608, 4 * GB)).toBe(true);
  });
});
