import type { TFunction } from "i18next";

export type ChartKind = "cpu" | "memory" | "players";
export type DurationUnit = "minutes" | "hours" | "days";
export type ChartWindows = Record<ChartKind, number>;

export const MIN_CHART_MINUTES = 5;
export const DEFAULT_CHART_MINUTES = 60;
export const MAX_OVERVIEW_MINUTES = 24 * 60;
export const MAX_RETENTION_MINUTES = 9_999 * 24 * 60;

const STORAGE_KEY = "minehost.overview.chartWindows";
const DEFAULT_WINDOWS: ChartWindows = { cpu: 60, memory: 60, players: 60 };

export function clampOverviewMinutes(minutes: number): number {
  if (!Number.isFinite(minutes)) return DEFAULT_CHART_MINUTES;
  return Math.min(MAX_OVERVIEW_MINUTES, Math.max(MIN_CHART_MINUTES, Math.round(minutes)));
}

export function loadChartWindows(): ChartWindows {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_WINDOWS };
    const parsed = JSON.parse(raw) as Partial<Record<ChartKind, unknown>>;
    return {
      cpu: typeof parsed.cpu === "number" ? clampOverviewMinutes(parsed.cpu) : DEFAULT_CHART_MINUTES,
      memory: typeof parsed.memory === "number" ? clampOverviewMinutes(parsed.memory) : DEFAULT_CHART_MINUTES,
      players: typeof parsed.players === "number" ? clampOverviewMinutes(parsed.players) : DEFAULT_CHART_MINUTES,
    };
  } catch {
    return { ...DEFAULT_WINDOWS };
  }
}

export function saveChartWindow(kind: ChartKind, minutes: number): ChartWindows {
  const next = { ...loadChartWindows(), [kind]: clampOverviewMinutes(minutes) };
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    // Bỏ qua khi trình duyệt chặn bộ nhớ cục bộ.
  }
  return next;
}

export function toDurationParts(minutes: number): { value: number; unit: DurationUnit } {
  const m = Math.max(1, Math.round(minutes));
  if (m % (24 * 60) === 0) return { value: m / (24 * 60), unit: "days" };
  if (m % 60 === 0) return { value: m / 60, unit: "hours" };
  return { value: m, unit: "minutes" };
}

export function fromDurationParts(value: number, unit: DurationUnit): number {
  const factor = unit === "days" ? 24 * 60 : unit === "hours" ? 60 : 1;
  return Math.round(value * factor);
}

export function formatSpanLabel(t: TFunction, minutes: number): string {
  if (minutes % (24 * 60) === 0) return t("duration.days", { count: minutes / (24 * 60) });
  if (minutes % 60 === 0) return t("duration.hours", { count: minutes / 60 });
  if (minutes > 60) {
    const h = Math.floor(minutes / 60);
    const m = minutes % 60;
    return `${t("duration.hours", { count: h })} ${t("duration.minutes", { count: m })}`;
  }
  return t("duration.minutes", { count: minutes });
}

export function formatChartTitle(t: TFunction, kind: ChartKind, minutes: number): string {
  if (minutes === DEFAULT_CHART_MINUTES) {
    if (kind === "cpu") return t("overview.cpuChart");
    if (kind === "memory") return t("overview.memoryChart");
    return t("overview.playersChart");
  }
  const metric = kind === "cpu" ? t("overview.cpu") : kind === "memory" ? t("overview.memory") : t("overview.players");
  return t("overview.chartRange", { metric, span: formatSpanLabel(t, minutes) });
}

export function toDayKey(ms: number): string {
  const d = new Date(ms);
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export function dayBounds(key: string): { start: number; end: number } {
  const [y, m, d] = key.split("-").map(Number);
  const start = new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1, 0, 0, 0, 0).getTime();
  const end = new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1, 23, 59, 59, 999).getTime();
  return { start, end };
}

export function shiftDay(key: string, delta: number): string {
  const { start } = dayBounds(key);
  const d = new Date(start);
  d.setDate(d.getDate() + delta);
  return toDayKey(d.getTime());
}

export function formatDayShort(key: string): string {
  const [, m, d] = key.split("-");
  return `${d ?? ""}/${m ?? ""}`;
}

export function listAvailableDays(oldestAt: string | null | undefined, nowMs: number): string[] {
  const todayKey = toDayKey(nowMs);
  const oldestMs = oldestAt ? Date.parse(oldestAt) : nowMs;
  const oldestKey = Number.isFinite(oldestMs) ? toDayKey(oldestMs) : todayKey;
  const days: string[] = [todayKey];
  let cursor = todayKey;
  for (let i = 0; i < 365 && cursor > oldestKey; i += 1) {
    cursor = shiftDay(cursor, -1);
    days.push(cursor);
  }
  return days;
}
