import { Columns2, Rows2, Settings } from "lucide-react";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import type { Server, ServerMetrics } from "../../../api/client";
import { formatBytes } from "../../../format";
import { Card, Columns, IconButton, LineChart, SegmentedControl, Stack, type RectBox } from "../../../ui";
import { usePlayers } from "../players/usePlayers";
import { ChartDetailDialog, samplesToPoints } from "./ChartDetailDialog";
import { loadChartLayout, saveChartLayout, useWideScreen, type ChartLayout } from "./chartLayout";
import { formatChartTitle, loadChartWindows, saveChartWindow, type ChartKind, type ChartWindows } from "./chartPrefs";
import { ChartSettingsDialog } from "./ChartSettingsDialog";
import { mergeLiveSample, useWindowMetrics } from "./metricsLive";

interface OverviewChartsProps {
  server: Server;
  liveMetrics: ServerMetrics | undefined;
  now: number;
}

const CHART_KINDS: ChartKind[] = ["cpu", "memory", "players"];

/** Biểu đồ CPU, RAM, người chơi trên trang Tổng quan: bấm thẻ để xem chi tiết, bánh răng để cài đặt. */
export function OverviewCharts({ server, liveMetrics, now }: OverviewChartsProps) {
  const { t, i18n } = useTranslation();
  const [layout, setLayout] = useState<ChartLayout>(loadChartLayout);
  const [windows, setWindows] = useState<ChartWindows>(loadChartWindows);
  const [detail, setDetail] = useState<{ kind: ChartKind; rect: RectBox | null } | null>(null);
  const [settingsKind, setSettingsKind] = useState<ChartKind | null>(null);
  const wide = useWideScreen();
  const players = usePlayers(server.id, server.state);

  function changeLayout(next: ChartLayout) {
    setLayout(next);
    saveChartLayout(next);
  }

  function handleSaveWindow(kind: ChartKind, minutes: number) {
    setWindows(saveChartWindow(kind, minutes));
  }

  const retentionMinutes = server.metrics_retention_minutes ?? liveMetrics?.retention_minutes ?? 60;
  const maxPlayers = players.data?.max_players;
  const detailTitle = detail ? formatChartTitle(t, detail.kind, windows[detail.kind]) : "";
  const settingsTitle = settingsKind ? formatChartTitle(t, settingsKind, windows[settingsKind]) : "";

  return (
    <Stack gap={3}>
      {wide && (
        <Stack direction="row" justify="end">
          <SegmentedControl
            label={t("overview.chartLayout")}
            value={layout}
            onChange={changeLayout}
            options={[
              { value: "side", label: t("overview.layoutSide"), icon: Columns2 },
              { value: "stacked", label: t("overview.layoutStacked"), icon: Rows2 },
            ]}
          />
        </Stack>
      )}
      <Columns count={layout === "side" ? 3 : 1}>
        {CHART_KINDS.map((kind) => (
          <OverviewChartCard
            key={kind}
            kind={kind}
            server={server}
            liveMetrics={liveMetrics}
            maxPlayers={maxPlayers}
            now={now}
            windowMinutes={windows[kind]}
            locale={i18n.language}
            onOpenDetail={(rect) => setDetail({ kind, rect: rect ?? null })}
            onOpenSettings={() => setSettingsKind(kind)}
          />
        ))}
      </Columns>

      {detail && (
        <ChartDetailDialog
          open
          onClose={() => setDetail(null)}
          originRect={detail.rect}
          kind={detail.kind}
          title={detailTitle}
          server={server}
          liveMetrics={liveMetrics}
          maxPlayers={maxPlayers}
          now={now}
          initialMinutes={windows[detail.kind]}
        />
      )}

      {settingsKind && (
        <ChartSettingsDialog
          open
          onClose={() => setSettingsKind(null)}
          kind={settingsKind}
          title={settingsTitle}
          server={server}
          windowMinutes={windows[settingsKind]}
          retentionMinutes={retentionMinutes}
          onSaveWindow={handleSaveWindow}
        />
      )}
    </Stack>
  );
}

interface OverviewChartCardProps {
  kind: ChartKind;
  server: Server;
  liveMetrics: ServerMetrics | undefined;
  maxPlayers: number | undefined;
  now: number;
  windowMinutes: number;
  locale: string;
  onOpenDetail: (rect?: RectBox) => void;
  onOpenSettings: () => void;
}

function OverviewChartCard({
  kind,
  server,
  liveMetrics,
  maxPlayers,
  now,
  windowMinutes,
  locale,
  onOpenDetail,
  onOpenSettings,
}: OverviewChartCardProps) {
  const { t } = useTranslation();
  const needsLongWindow = windowMinutes > 60;
  const longQuery = useWindowMetrics(server.id, windowMinutes, needsLongWindow);
  const source = needsLongWindow ? mergeLiveSample(longQuery.data ?? liveMetrics, liveMetrics) : liveMetrics;
  const samples = source?.samples ?? [];
  const stepSeconds = source?.step_seconds ?? 5;
  const start = now - windowMinutes * 60_000;
  const points = samplesToPoints(kind, samples).filter((p) => p.t >= start);
  const title = formatChartTitle(t, kind, windowMinutes);
  const timeFmt = useMemo(() => new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }), [locale]);
  const ramMaxBytes = server.memory_mb ? server.memory_mb.max * 1024 * 1024 : undefined;
  const formatValue =
    kind === "cpu"
      ? (v: number) => `${Math.round(v)} %`
      : kind === "memory"
        ? (v: number) => formatBytes(v, locale)
        : (v: number) => String(Math.round(v));

  return (
    <Card
      title={title}
      onClick={onOpenDetail}
      clickLabel={t("overview.openDetails", { title })}
      actions={
        <IconButton
          icon={Settings}
          size="sm"
          label={t("overview.chartSettings")}
          onClick={(e) => {
            e.stopPropagation();
            onOpenSettings();
          }}
        />
      }
    >
      <LineChart
        label={title}
        points={points}
        start={start}
        end={now}
        yMax={kind === "cpu" ? 100 : kind === "memory" ? ramMaxBytes : maxPlayers}
        expandYMax={kind === "memory"}
        referenceValue={kind === "memory" ? ramMaxBytes : undefined}
        referenceLabel={kind === "memory" && ramMaxBytes ? t("overview.memoryLimitLabel", { max: formatBytes(ramMaxBytes, locale) }) : undefined}
        peakPrefix={t("overview.peakPrefix")}
        formatValue={formatValue}
        formatTime={(ms) => timeFmt.format(ms)}
        gapMs={Math.max(15_000, stepSeconds * 2_500)}
      />
    </Card>
  );
}
