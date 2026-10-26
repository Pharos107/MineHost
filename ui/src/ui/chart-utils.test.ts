import { describe, expect, it } from "vitest";

import { linePath, nearestIndex, niceMax, ticks } from "./chart-utils";

describe("trục y", () => {
  it("làm tròn trần lên số dễ đọc", () => {
    expect(niceMax(0.7)).toBe(1);
    expect(niceMax(3.2)).toBe(5);
    expect(niceMax(18)).toBe(20);
    expect(niceMax(21)).toBe(25);
    expect(niceMax(6100)).toBe(10000);
  });

  it("không có số dương thì trần là 1, để không chia cho 0", () => {
    expect(niceMax(0)).toBe(1);
    expect(niceMax(Number.NaN)).toBe(1);
  });

  it("chia đều từ 0 tới trần", () => {
    expect(ticks(100)).toEqual([0, 25, 50, 75, 100]);
  });
});

describe("điểm gần nhất", () => {
  const points = [10, 20, 30, 40].map((t) => ({ t, v: t }));

  it("chọn điểm có thời gian gần nhất, nằm giữa thì lấy điểm trước", () => {
    expect(nearestIndex(points, 24)).toBe(1);
    expect(nearestIndex(points, 26)).toBe(2);
    expect(nearestIndex(points, 25)).toBe(1);
  });

  it("ngoài hai đầu thì kẹp vào điểm đầu hoặc cuối; rỗng thì -1", () => {
    expect(nearestIndex(points, -5)).toBe(0);
    expect(nearestIndex(points, 99)).toBe(3);
    expect(nearestIndex([], 5)).toBe(-1);
  });
});

describe("đường vẽ", () => {
  it("ngắt nét khi hai điểm cách nhau quá xa", () => {
    const points = [
      { t: 0, v: 1 },
      { t: 5, v: 2 },
      { t: 100, v: 3 },
    ];
    expect(linePath(points, (t) => t, (v) => v, 10)).toBe("M0.0,1.0L5.0,2.0M100.0,3.0");
  });
});
