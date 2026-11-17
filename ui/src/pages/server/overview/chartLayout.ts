import type { ChartPoint } from "../../../ui";
import { useMediaQuery } from "../../../useMediaQuery";

export type ChartLayout = "side" | "stacked";

const STORAGE_KEY = "minehost.overview.charts";

/** Cách xếp biểu đồ đã chọn lần trước; trình duyệt chặn bộ nhớ thì xếp cạnh nhau. */
export function loadChartLayout(): ChartLayout {
  try {
    return localStorage.getItem(STORAGE_KEY) === "stacked" ? "stacked" : "side";
  } catch {
    return "side";
  }
}

export function saveChartLayout(layout: ChartLayout) {
  try {
    localStorage.setItem(STORAGE_KEY, layout);
  } catch {
    // Không lưu được thì chỉ mất lựa chọn khi tải lại trang.
  }
}

export interface ChartStats {
  last: number;
  average: number;
  max: number;
  min: number;
}

/** Số liệu cho hộp xem chi tiết. Chưa có điểm nào thì `undefined`. */
export function summarize(points: ChartPoint[]): ChartStats | undefined {
  const last = points.at(-1);
  if (!last) return undefined;
  const values = points.map((p) => p.v);
  const sum = values.reduce((total, v) => total + v, 0);
  return { last: last.v, average: sum / values.length, max: Math.max(...values), min: Math.min(...values) };
}

/** Màn hình đủ rộng để xếp biểu đồ cạnh nhau (cùng mốc 900 px với `Columns`). jsdom thì coi là rộng. */
export function useWideScreen(): boolean {
  return useMediaQuery("(min-width: 900px)", true);
}
