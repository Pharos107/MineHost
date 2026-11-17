import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent, type RefObject } from "react";

import { linePath, nearestIndex, niceMax, ticks, type ChartPoint } from "./chart-utils";
import { cx } from "./cx";
import styles from "./LineChart.module.css";

interface LineChartProps {
  /** Tên biểu đồ cho trình đọc màn hình, ví dụ "CPU trong 1 giờ qua". */
  label: string;
  /** Xếp theo thời gian tăng dần. */
  points: ChartPoint[];
  /** Khung thời gian hiển thị (mili giây). Bỏ trống thì lấy từ điểm đầu và cuối. */
  start?: number;
  end?: number;
  /** Trần trục y cố định; bật `expandYMax` thì tự nới khi có điểm vượt mức này. */
  yMax?: number;
  /** Tự nâng trần trục y nếu dữ liệu vượt `yMax` (dùng cho RAM khi RSS vượt `-Xmx`). */
  expandYMax?: boolean;
  /** Đường tham chiếu ngang (ví dụ mức RAM đã đặt). */
  referenceValue?: number;
  referenceLabel?: string;
  /** Nhãn tiền tố cho đỉnh khi điểm đã gộp mẫu (mặc định "Max"). */
  peakPrefix?: string;
  formatValue: (value: number) => string;
  formatTime: (time: number) => string;
  /** Khoảng cách lớn hơn mức này giữa hai điểm thì ngắt nét. */
  gapMs?: number;
  height?: number;
  /** Bật kéo vùng / lăn chuột để zoom và Shift+kéo để trượt. */
  onZoomRange?: (start: number, end: number) => void;
  onResetZoom?: () => void;
}

const PAD = { top: 8, right: 8, bottom: 22, left: 52 };
const FALLBACK_WIDTH = 600;
const MIN_ZOOM_MS = 60_000;
const TIME_TICKS = [
  { at: 0, anchor: "start" },
  { at: 0.5, anchor: "middle" },
  { at: 1, anchor: "end" },
] as const;

type DragState = { mode: "brush"; anchorX: number; currentX: number } | { mode: "pan"; anchorX: number; start0: number; end0: number };

/**
 * Biểu đồ đường một chuỗi số, vẽ bằng SVG. Hỗ trợ tooltip theo con trỏ/bàn phím,
 * tự nới trần khi vượt ngưỡng và zoom/pan tương tác khi mở chi tiết.
 */
export function LineChart(props: LineChartProps) {
  const {
    label,
    points,
    expandYMax = false,
    referenceValue,
    referenceLabel,
    peakPrefix = "Max",
    formatValue,
    formatTime,
    gapMs = 15_000,
    height = 160,
    onZoomRange,
    onResetZoom,
  } = props;
  const host = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const width = useWidth(host);
  const [hover, setHover] = useState<number | null>(null);
  const [drag, setDrag] = useState<DragState | null>(null);

  const start = props.start ?? points[0]?.t ?? 0;
  const end = props.end ?? points.at(-1)?.t ?? start + 1;
  const span = Math.max(1, end - start);
  const maxPoint = Math.max(0, ...points.map((p) => Math.max(p.v, p.vMax ?? p.v)));
  const yMax = computeYMax(props.yMax, expandYMax, maxPoint);
  const plotW = Math.max(1, width - PAD.left - PAD.right);
  const plotH = Math.max(1, height - PAD.top - PAD.bottom);
  const x = (t: number) => PAD.left + ((t - start) / span) * plotW;
  const y = (v: number) => PAD.top + plotH - (Math.min(Math.max(v, 0), yMax) / yMax) * plotH;
  const hasPeak = points.some((p) => p.vMax !== undefined && p.vMax > p.v);
  const point = hover === null ? undefined : points[hover];
  const last = points.at(-1);

  useWheelZoom(svgRef, onZoomRange, start, end, plotW);

  function pointerX(clientX: number, target: SVGSVGElement): number {
    const rect = target.getBoundingClientRect();
    return Math.min(Math.max(clientX - rect.left, PAD.left), PAD.left + plotW);
  }

  function onPointerDown(e: PointerEvent<SVGSVGElement>) {
    if (!onZoomRange || e.button !== 0) return;
    const px = pointerX(e.clientX, e.currentTarget);
    e.currentTarget.setPointerCapture?.(e.pointerId);
    setDrag(e.shiftKey ? { mode: "pan", anchorX: px, start0: start, end0: end } : { mode: "brush", anchorX: px, currentX: px });
  }

  function onPointerMove(e: PointerEvent<SVGSVGElement>) {
    const rect = e.currentTarget.getBoundingClientRect();
    const t = start + ((e.clientX - rect.left - PAD.left) / plotW) * span;
    setHover(nearestIndex(points, t));
    if (!drag || !onZoomRange) return;
    const px = pointerX(e.clientX, e.currentTarget);
    if (drag.mode === "brush") {
      setDrag({ ...drag, currentX: px });
      return;
    }
    const shiftMs = ((drag.anchorX - px) / plotW) * (drag.end0 - drag.start0);
    onZoomRange(drag.start0 + shiftMs, drag.end0 + shiftMs);
  }

  function onPointerUp(e: PointerEvent<SVGSVGElement>) {
    if (!drag) return;
    if (e.currentTarget.hasPointerCapture?.(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
    if (drag.mode === "brush" && onZoomRange) {
      const left = Math.min(drag.anchorX, drag.currentX);
      const right = Math.max(drag.anchorX, drag.currentX);
      if (right - left >= 8) {
        const t0 = start + ((left - PAD.left) / plotW) * span;
        const t1 = start + ((right - PAD.left) / plotW) * span;
        if (t1 - t0 >= MIN_ZOOM_MS) onZoomRange(t0, t1);
      }
    }
    setDrag(null);
  }

  function onKeyDown(e: KeyboardEvent<SVGSVGElement>) {
    if (points.length === 0 || (e.key !== "ArrowLeft" && e.key !== "ArrowRight")) return;
    e.preventDefault();
    const step = e.key === "ArrowLeft" ? -1 : 1;
    setHover((current) => Math.min(points.length - 1, Math.max(0, (current ?? points.length - 1) + step)));
  }

  const brushX = drag?.mode === "brush" ? Math.min(drag.anchorX, drag.currentX) : 0;
  const brushW = drag?.mode === "brush" ? Math.abs(drag.currentX - drag.anchorX) : 0;
  const showRef = referenceValue !== undefined && referenceValue > 0 && referenceValue <= yMax;

  return (
    <div className={styles.chart} ref={host}>
      <svg
        ref={svgRef}
        className={cx(onZoomRange && styles.interactiveSvg)}
        width={width}
        height={height}
        role="img"
        aria-label={last ? `${label}: ${formatValue(last.v)}` : label}
        tabIndex={0}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={() => setDrag(null)}
        onPointerLeave={() => setHover(null)}
        onDoubleClick={() => onResetZoom?.()}
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
          <text key={anchor} className={styles.axis} x={x(start + at * span)} y={height - 6} textAnchor={anchor}>
            {formatTime(start + at * span)}
          </text>
        ))}
        {showRef && (
          <g>
            <line className={styles.referenceLine} x1={PAD.left} x2={width - PAD.right} y1={y(referenceValue)} y2={y(referenceValue)} />
            {referenceLabel && (
              <text className={styles.referenceLabel} x={width - PAD.right - 4} y={y(referenceValue) - 4} textAnchor="end">
                {referenceLabel}
              </text>
            )}
          </g>
        )}
        {hasPeak && <path className={styles.peakLine} d={linePath(points, x, y, gapMs, (p) => p.vMax ?? p.v)} />}
        <path className={styles.line} d={linePath(points, x, y, gapMs)} />
        {drag?.mode === "brush" && brushW >= 2 && <rect className={styles.brush} x={brushX} y={PAD.top} width={brushW} height={plotH} />}
        {point && (
          <g>
            <line className={styles.crosshair} x1={x(point.t)} x2={x(point.t)} y1={PAD.top} y2={PAD.top + plotH} />
            <circle className={styles.dot} cx={x(point.t)} cy={y(point.v)} r={4} />
          </g>
        )}
      </svg>
      {point && (
        <div className={styles.tooltip} style={{ left: Math.min(Math.max(x(point.t), 60), width - 60) }} role="status">
          <span className={styles.tooltipValue}>{formatValue(point.v)}</span>
          {point.vMax !== undefined && point.vMax > point.v && (
            <span className={styles.tooltipPeak}>{`${peakPrefix}: ${formatValue(point.vMax)}`}</span>
          )}
          <span className={styles.tooltipTime}>{formatTime(point.t)}</span>
        </div>
      )}
    </div>
  );
}

function computeYMax(fixedYMax: number | undefined, expand: boolean, maxPoint: number): number {
  if (fixedYMax === undefined) return niceMax(maxPoint);
  if (expand && maxPoint > fixedYMax) return niceMax(maxPoint);
  return fixedYMax;
}

function useWheelZoom(
  svgRef: RefObject<SVGSVGElement | null>,
  onZoomRange: ((start: number, end: number) => void) | undefined,
  start: number,
  end: number,
  plotW: number,
) {
  useEffect(() => {
    const svg = svgRef.current;
    if (!svg || !onZoomRange) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const rect = svg.getBoundingClientRect();
      const ratio = Math.min(Math.max((e.clientX - rect.left - PAD.left) / Math.max(1, plotW), 0), 1);
      const span = Math.max(1, end - start);
      const anchor = start + ratio * span;
      const factor = e.deltaY < 0 ? 0.75 : 1.35;
      const nextSpan = Math.max(MIN_ZOOM_MS, span * factor);
      onZoomRange(anchor - ratio * nextSpan, anchor + (1 - ratio) * nextSpan);
    };
    svg.addEventListener("wheel", onWheel, { passive: false });
    return () => svg.removeEventListener("wheel", onWheel);
  }, [svgRef, onZoomRange, start, end, plotW]);
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
