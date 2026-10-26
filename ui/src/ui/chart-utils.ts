/** Một điểm trên biểu đồ: `t` là mili giây (Unix), `v` là giá trị. */
export interface ChartPoint {
  t: number;
  v: number;
}

/** Làm tròn trần trục y lên 1, 2, 2.5 hoặc 5 × 10ⁿ để vạch chia là số dễ đọc. */
export function niceMax(value: number): number {
  if (!(value > 0) || !Number.isFinite(value)) return 1;
  const exponent = Math.floor(Math.log10(value));
  const base = 10 ** exponent;
  const step = [1, 2, 2.5, 5, 10].find((m) => m * base >= value) ?? 10;
  return step * base;
}

/** Các mốc chia đều từ 0 tới `max`, gồm cả hai đầu. */
export function ticks(max: number, count = 4): number[] {
  return Array.from({ length: count + 1 }, (_, i) => (max * i) / count);
}

/** Chỉ số của điểm có thời gian gần `t` nhất; danh sách rỗng thì `-1`. Điểm phải xếp theo `t` tăng dần. */
export function nearestIndex(points: ChartPoint[], t: number): number {
  if (points.length === 0) return -1;
  const timeAt = (i: number) => points[i]?.t ?? Number.NaN;
  let lo = 0;
  let hi = points.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (timeAt(mid) < t) lo = mid + 1;
    else hi = mid;
  }
  if (lo > 0 && Math.abs(timeAt(lo - 1) - t) <= Math.abs(timeAt(lo) - t)) return lo - 1;
  return lo;
}

/**
 * Đường `d` cho `<path>`. Hai điểm cách nhau quá `gapMs` (server tắt rồi bật lại) thì ngắt nét,
 * để biểu đồ không vẽ một đường thẳng giả qua khoảng không có số liệu.
 */
export function linePath(
  points: ChartPoint[],
  x: (t: number) => number,
  y: (v: number) => number,
  gapMs: number,
): string {
  return points
    .map((p, i) => {
      const previous = points[i - 1];
      const jump = previous === undefined || p.t - previous.t > gapMs;
      return `${jump ? "M" : "L"}${x(p.t).toFixed(1)},${y(p.v).toFixed(1)}`;
    })
    .join("");
}
