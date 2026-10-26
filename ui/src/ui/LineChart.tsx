import { useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent, type RefObject } from "react";

import { linePath, nearestIndex, niceMax, ticks, type ChartPoint } from "./chart-utils";
import styles from "./LineChart.module.css";

interface LineChartProps {
  /** Tên biểu đồ cho trình đọc màn hình, ví dụ "CPU trong 1 giờ qua". */
  label: string;
  /** Xếp theo thời gian tăng dần. */
  points: ChartPoint[];
  /** Khung thời gian hiển thị (mili giây). Bỏ trống thì lấy từ điểm đầu và cuối. */
  start?: number;
  end?: number;
  /** Trần trục y cố định (ví dụ 100 cho %, RAM tối đa đã đặt); bỏ trống thì tự làm tròn từ số lớn nhất. */
  yMax?: number;
  formatValue: (value: number) => string;
  formatTime: (time: number) => string;
  /** Khoảng cách lớn hơn mức này giữa hai điểm thì ngắt nét (server đã tắt một lúc). */
  gapMs?: number;
  height?: number;
}

const PAD = { top: 8, right: 8, bottom: 22, left: 52 };
const FALLBACK_WIDTH = 600;
/** Ba mốc giờ dưới trục x: đầu, giữa, cuối khung thời gian. */
const TIME_TICKS = [
  { at: 0, anchor: "start" },
  { at: 0.5, anchor: "middle" },
  { at: 1, anchor: "end" },
] as const;

/**
 * Biểu đồ đường một chuỗi số, vẽ bằng SVG. Mỗi biểu đồ chỉ một đơn vị (một trục y); muốn so CPU
 * với RAM thì đặt hai biểu đồ cạnh nhau. Rê chuột hoặc dùng phím mũi tên để xem từng điểm.
 */
export function LineChart(props: LineChartProps) {
  const { label, points, formatValue, formatTime, gapMs = 15_000, height = 160 } = props;
  const host = useRef<HTMLDivElement>(null);
  const width = useWidth(host);
  const [hover, setHover] = useState<number | null>(null);

  const start = props.start ?? points[0]?.t ?? 0;
  const end = props.end ?? points.at(-1)?.t ?? start + 1;
  const yMax = props.yMax ?? niceMax(Math.max(0, ...points.map((p) => p.v)));
  const plotW = Math.max(1, width - PAD.left - PAD.right);
  const plotH = Math.max(1, height - PAD.top - PAD.bottom);
  const x = (t: number) => PAD.left + ((t - start) / Math.max(1, end - start)) * plotW;
  const y = (v: number) => PAD.top + plotH - (Math.min(v, yMax) / yMax) * plotH;
  const point = hover === null ? undefined : points[hover];
  const last = points.at(-1);

  function onPointerMove(e: PointerEvent<SVGSVGElement>) {
    const rect = e.currentTarget.getBoundingClientRect();
    const t = start + ((e.clientX - rect.left - PAD.left) / plotW) * (end - start);
    setHover(nearestIndex(points, t));
  }

  function onKeyDown(e: KeyboardEvent<SVGSVGElement>) {
    if (points.length === 0 || (e.key !== "ArrowLeft" && e.key !== "ArrowRight")) return;
    e.preventDefault();
    const step = e.key === "ArrowLeft" ? -1 : 1;
    setHover((current) => Math.min(points.length - 1, Math.max(0, (current ?? points.length - 1) + step)));
  }

  return (
    <div className={styles.chart} ref={host}>
      <svg
        width={width}
        height={height}
        role="img"
        aria-label={last ? `${label}: ${formatValue(last.v)}` : label}
        tabIndex={0}
        onPointerMove={onPointerMove}
        onPointerLeave={() => setHover(null)}
        onKeyDown={onKeyDown}
        onBlur={() => setHover(null)}
      >
        {ticks(yMax).map((tick) => (
          <g key={tick}>
            <line className={styles.grid} x1={PAD.left} x2={width - PAD.right} y1={y(tick)} y2={y(tick)} />
            <text className={styles.axis} x={PAD.left - 8} y={y(tick)} textAnchor="end" dominantBaseline="middle">
              {formatValue(tick)}
            </text>
          </g>
        ))}
        {TIME_TICKS.map(({ at, anchor }) => (
          <text key={anchor} className={styles.axis} x={x(start + at * (end - start))} y={height - 6} textAnchor={anchor}>
            {formatTime(start + at * (end - start))}
          </text>
        ))}
        <path className={styles.line} d={linePath(points, x, y, gapMs)} />
        {point && (
          <g>
            <line className={styles.crosshair} x1={x(point.t)} x2={x(point.t)} y1={PAD.top} y2={PAD.top + plotH} />
            <circle className={styles.dot} cx={x(point.t)} cy={y(point.v)} r={4} />
          </g>
        )}
      </svg>
      {point && (
        <div
          className={styles.tooltip}
          style={{ left: Math.min(Math.max(x(point.t), 60), width - 60) }}
          role="status"
        >
          <span className={styles.tooltipValue}>{formatValue(point.v)}</span>
          <span className={styles.tooltipTime}>{formatTime(point.t)}</span>
        </div>
      )}
    </div>
  );
}

/** Độ rộng thật của vùng vẽ, cập nhật khi đổi cỡ cửa sổ. jsdom không có `ResizeObserver` thì dùng số mặc định. */
function useWidth(host: RefObject<HTMLDivElement | null>): number {
  const [width, setWidth] = useState(FALLBACK_WIDTH);
  useLayoutEffect(() => {
    const element = host.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => setWidth(Math.round(entry?.contentRect.width ?? 0) || FALLBACK_WIDTH));
    observer.observe(element);
    return () => observer.disconnect();
  }, [host]);
  return width;
}
