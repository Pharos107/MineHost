import { ChevronLeft, ChevronRight, RotateCcw } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import type { MetricSample, Server, ServerMetrics } from "../../../api/client";
import { formatBytes } from "../../../format";
import { Button, Dialog, DialogBody, Hint, IconButton, LineChart, Select, StatGrid, StatTile, Stack, type ChartPoint, type RectBox } from "../../../ui";
import { summarize } from "./chartLayout";
import { dayBounds, formatDayShort, listAvailableDays, toDayKey, type ChartKind } from "./chartPrefs";
import { mergeLiveSample, useDetailMetrics } from "./metricsLive";

type LiveSpan = "15" | "30" | "60" | "360" | "1440" | "all";
type ViewMode = { kind: "live"; span: LiveSpan } | { kind: "day"; day: string };

interface ChartDetailDialogProps {
  open: boolean;
  onClose: () => void;
  originRect: RectBox | null;
  kind: ChartKind;
  title: string;
  server: Server;
  liveMetrics: ServerMetrics | undefined;
  maxPlayers: number | undefined;
  now: number;
  initialMinutes: number;
}

const LIVE_SPANS: { value: LiveSpan; labelKey: string }[] = [
  { value: "15", labelKey: "overview.preset15m" },
  { value: "30", labelKey: "overview.preset30m" },
  { value: "60", labelKey: "overview.preset1h" },
  { value: "360", labelKey: "overview.preset6h" },
  { value: "1440", labelKey: "overview.preset1d" },
  { value: "all", labelKey: "overview.allData" },
];

export function ChartDetailDialog(props: ChartDetailDialogProps) {
  return (
    <Dialog open={props.open} onClose={props.onClose} title={props.title} size="xl" originRect={props.originRect}>
      {props.open && <DetailBody {...props} />}
    </Dialog>
  );
}

function defaultSpan(minutes: number): LiveSpan {
  if (minutes <= 15) return "15";
  if (minutes <= 30) return "30";
  if (minutes <= 60) return "60";
  if (minutes <= 360) return "360";
  return "1440";
}

function DetailBody({ kind, title, server, liveMetrics, maxPlayers, now, initialMinutes }: ChartDetailDialogProps) {
  const { t, i18n } = useTranslation();
  const [mode, setMode] = useState<ViewMode>(() => ({ kind: "live", span: defaultSpan(initialMinutes) }));
  const [zoom, setZoom] = useState<{ start: number; end: number } | null>(null);
  const debouncedZoom = useDebouncedValue(zoom, 180);

  const oldestIso = liveMetrics?.oldest_at;
  const oldestMs = oldestIso ? Date.parse(oldestIso) : now - 3_600_000;
  const baseRange = computeBaseRange(mode, now, oldestMs);
  const viewStart = zoom ? zoom.start : baseRange.start;
  const viewEnd = zoom ? zoom.end : baseRange.end;

  // Làm tròn mốc hỏi API theo phút ở chế độ trực tiếp để mẫu WebSocket mỗi 5 giây không kích hoạt refetch liên tục.
  const stableNow = Math.floor(now / 60_000) * 60_000;
  const stableBase = computeBaseRange(mode, stableNow, oldestMs);
  const queryStart = debouncedZoom ? debouncedZoom.start : stableBase.start;
  const queryEnd = debouncedZoom ? debouncedZoom.end : stableBase.end;
  const useLiveOnly = mode.kind === "live" && (mode.span === "15" || mode.span === "30" || mode.span === "60") && !debouncedZoom;

  const fetched = useDetailMetrics(
    server.id,
    new Date(queryStart).toISOString(),
    mode.kind === "day" || debouncedZoom ? new Date(queryEnd).toISOString() : undefined,
    480,
    !useLiveOnly,
    mode.kind === "live" && !debouncedZoom,
    mode.kind === "live" || Boolean(debouncedZoom),
  );

  const isPastDay = mode.kind === "day" && mode.day !== toDayKey(now);
  const activeData = useLiveOnly
    ? liveMetrics
    : mode.kind === "live"
      ? mergeLiveSample(fetched.data ?? liveMetrics, liveMetrics)
      : isPastDay
        ? fetched.data
        : mergeLiveSample(fetched.data ?? liveMetrics, liveMetrics);

  const samples = activeData?.samples ?? [];
  const stepSeconds = activeData?.step_seconds ?? 5;
  const allPoints = samplesToPoints(kind, samples);
  const visiblePoints = allPoints.filter((p) => p.t >= viewStart && p.t <= viewEnd);
  const stats = summarize(visiblePoints);

  const days = listAvailableDays(activeData?.oldest_at ?? oldestIso, now);
  const currentDay = mode.kind === "day" ? mode.day : toDayKey(now);
  const dayIdx = days.indexOf(currentDay);
  const hasPrevDay = mode.kind === "live" ? days.length > 1 : dayIdx >= 0 && dayIdx < days.length - 1;
  const hasNextDay = mode.kind === "day";

  const formatValue = makeFormatter(kind, i18n.language);
  const multiDay = viewEnd - viewStart > 36 * 3_600_000;
  const timeFmt = useMemo(
    () =>
      new Intl.DateTimeFormat(
        i18n.language,
        multiDay ? { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" } : { hour: "2-digit", minute: "2-digit" },
      ),
    [i18n.language, multiDay],
  );
  const ramMaxBytes = server.memory_mb ? server.memory_mb.max * 1024 * 1024 : undefined;
  const show = (value: number | undefined) => (value === undefined ? "—" : formatValue(value));

  function selectDay(target: string) {
    setZoom(null);
    if (target === "live") {
      setMode({ kind: "live", span: "60" });
      return;
    }
    setMode({ kind: "day", day: target });
  }

  function stepDay(delta: -1 | 1) {
    setZoom(null);
    if (mode.kind === "live") {
      if (delta === -1 && days.length > 1) setMode({ kind: "day", day: days[1]! });
      return;
    }
    const nextIdx = dayIdx - delta;
    if (nextIdx < 0) {
      setMode({ kind: "live", span: "60" });
      return;
    }
    if (nextIdx < days.length) setMode({ kind: "day", day: days[nextIdx]! });
  }

  function handleZoom(nextStart: number, nextEnd: number) {
    const minT = mode.kind === "day" ? baseRange.start : Math.min(oldestMs, baseRange.start);
    const maxT = mode.kind === "day" ? baseRange.end : now;
    setZoom(clampRange(nextStart, nextEnd, minT, maxT));
  }

  return (
    <DialogBody>
      <Stack gap={4}>
        <Stack direction="row" gap={2} justify="between" align="center" wrap>
          <Stack direction="row" gap={1} align="center">
            <IconButton icon={ChevronLeft} size="sm" label={t("overview.prevDay")} disabled={!hasPrevDay} onClick={() => stepDay(-1)} />
            <Select aria-label={t("overview.selectDay")} value={mode.kind === "live" ? "live" : mode.day} onChange={(e) => selectDay(e.target.value)}>
              <option value="live">{`${t("overview.live")} · ${formatDayShort(toDayKey(now))}`}</option>
              {days.map((d) => (
                <option key={d} value={d}>
                  {formatDayShort(d)}
                </option>
              ))}
            </Select>
            <IconButton icon={ChevronRight} size="sm" label={t("overview.nextDay")} disabled={!hasNextDay} onClick={() => stepDay(1)} />
          </Stack>

          <Stack direction="row" gap={1} align="center" wrap>
            {LIVE_SPANS.map((item) => (
              <Button
                key={item.value}
                size="sm"
                variant={mode.kind === "live" && mode.span === item.value && !zoom ? "primary" : "ghost"}
                onClick={() => {
                  setZoom(null);
                  setMode({ kind: "live", span: item.value });
                }}
              >
                {t(item.labelKey)}
              </Button>
            ))}
            {zoom && (
              <Button size="sm" variant="secondary" icon={RotateCcw} onClick={() => setZoom(null)}>
                {t("overview.resetZoom")}
              </Button>
            )}
          </Stack>
        </Stack>

        <StatGrid>
          <StatTile label={t("overview.now")} value={show(stats?.last)} />
          <StatTile label={t("overview.average")} value={show(stats?.average)} />
          <StatTile label={t("overview.peak")} value={show(stats?.max)} />
          <StatTile label={t("overview.lowest")} value={show(stats?.min)} />
        </StatGrid>

        <LineChart
          label={title}
          points={visiblePoints}
          start={viewStart}
          end={viewEnd}
          yMax={kind === "cpu" ? 100 : kind === "memory" ? ramMaxBytes : maxPlayers}
          expandYMax={kind === "memory"}
          referenceValue={kind === "memory" ? ramMaxBytes : undefined}
          referenceLabel={kind === "memory" && ramMaxBytes ? t("overview.memoryLimitLabel", { max: formatBytes(ramMaxBytes, i18n.language) }) : undefined}
          peakPrefix={t("overview.peakPrefix")}
          formatValue={formatValue}
          formatTime={(ms) => timeFmt.format(ms)}
          gapMs={Math.max(15_000, stepSeconds * 2_500)}
          height={400}
          onZoomRange={handleZoom}
          onResetZoom={() => setZoom(null)}
        />
        <Hint>{t("overview.zoomHint")}</Hint>
      </Stack>
    </DialogBody>
  );
}

function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), delayMs);
    return () => clearTimeout(id);
  }, [value, delayMs]);
  return debounced;
}

function computeBaseRange(mode: ViewMode, now: number, oldestMs: number): { start: number; end: number } {
  if (mode.kind === "day") return dayBounds(mode.day);
  if (mode.span === "all") {
    const start = Number.isFinite(oldestMs) && oldestMs < now ? oldestMs : now - 3_600_000;
    return { start, end: now };
  }
  return { start: now - Number(mode.span) * 60_000, end: now };
}

function clampRange(nextStart: number, nextEnd: number, minT: number, maxT: number): { start: number; end: number } {
  const span = Math.max(60_000, Math.min(nextEnd - nextStart, Math.max(60_000, maxT - minT)));
  let start = nextStart;
  let end = start + span;
  if (start < minT) {
    start = minT;
    end = Math.min(maxT, start + span);
  }
  if (end > maxT) {
    end = maxT;
    start = Math.max(minT, end - span);
  }
  return { start, end };
}

export function samplesToPoints(kind: ChartKind, samples: MetricSample[]): ChartPoint[] {
  const at = (s: MetricSample) => Date.parse(s.at);
  if (kind === "cpu") return samples.map((s) => ({ t: at(s), v: s.cpu_percent, vMax: s.cpu_max ?? undefined }));
  if (kind === "memory") return samples.map((s) => ({ t: at(s), v: s.memory_bytes, vMax: s.memory_max_bytes ?? undefined }));
  return samples.flatMap((s) => (s.players == null ? [] : [{ t: at(s), v: s.players, vMax: s.players_max ?? undefined }]));
}

function makeFormatter(kind: ChartKind, locale: string): (value: number) => string {
  if (kind === "cpu") return (v) => `${Math.round(v)} %`;
  if (kind === "memory") return (v) => formatBytes(v, locale);
  return (v) => String(Math.round(v));
}
