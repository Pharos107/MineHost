import { Columns2, Maximize2, Rows2 } from "lucide-react";
import type { TFunction } from "i18next";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import type { MetricSample, Server } from "../../../api/client";
import { formatBytes } from "../../../format";
import { Card, Columns, Dialog, DialogBody, IconButton, LineChart, SegmentedControl, Stack, StatGrid, StatTile, type ChartPoint } from "../../../ui";
import { usePlayers } from "../players/usePlayers";
import { loadChartLayout, saveChartLayout, summarize, useWideScreen, type ChartLayout } from "./chartLayout";

const HOUR_MS = 3_600_000;

interface ChartSpec {
  title: string;
  points: ChartPoint[];
  yMax?: number;
  formatValue: (value: number) => string;
}

interface OverviewChartsProps {
  server: Server;
  samples: MetricSample[];
  now: number;
}

/** CPU, RAM và số người chơi trong 1 giờ. Mẫu chưa có số người chơi (`null`) thì bỏ qua điểm đó. */
function chartSpecs(t: TFunction, locale: string, server: Server, samples: MetricSample[], maxPlayers: number | undefined): ChartSpec[] {
  const at = (s: MetricSample) => Date.parse(s.at);
  const players = samples.flatMap((s) => (s.players == null ? [] : [{ t: at(s), v: s.players }]));
  return [
    { title: t("overview.cpuChart"), points: samples.map((s) => ({ t: at(s), v: s.cpu_percent })), yMax: 100, formatValue: (v) => `${Math.round(v)} %` },
    {
      title: t("overview.memoryChart"),
      points: samples.map((s) => ({ t: at(s), v: s.memory_bytes })),
      yMax: server.memory_mb ? server.memory_mb.max * 1024 * 1024 : undefined,
      formatValue: (v) => formatBytes(v, locale),
    },
    { title: t("overview.playersChart"), points: players, yMax: maxPlayers, formatValue: (v) => String(Math.round(v)) },
  ];
}

/** Biểu đồ CPU, RAM, người chơi 1 giờ qua: xếp cạnh nhau hoặc từng hàng (nhớ theo trình duyệt), mỗi biểu đồ mở to được. */
export function OverviewCharts({ server, samples, now }: OverviewChartsProps) {
  const { t, i18n } = useTranslation();
  const [layout, setLayout] = useState<ChartLayout>(loadChartLayout);
  const wide = useWideScreen();
  const players = usePlayers(server.id, server.state);
  const time = new Intl.DateTimeFormat(i18n.language, { hour: "2-digit", minute: "2-digit" });
  const charts = chartSpecs(t, i18n.language, server, samples, players.data?.max_players);
  const span = { start: now - HOUR_MS, end: now, formatTime: (ms: number) => time.format(ms) };

  function changeLayout(next: ChartLayout) {
    setLayout(next);
    saveChartLayout(next);
  }

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
        {charts.map((chart) => (
          <ChartCard key={chart.title} chart={chart} {...span} />
        ))}
      </Columns>
    </Stack>
  );
}

interface ChartCardProps {
  chart: ChartSpec;
  start: number;
  end: number;
  formatTime: (ms: number) => string;
}

function ChartCard({ chart, start, end, formatTime }: ChartCardProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const line = { label: chart.title, points: chart.points, start, end, yMax: chart.yMax, formatValue: chart.formatValue, formatTime };

  return (
    <Card title={chart.title} actions={<IconButton icon={Maximize2} size="sm" label={t("overview.details")} onClick={() => setOpen(true)} />}>
      <LineChart {...line} />
      <Dialog open={open} onClose={() => setOpen(false)} title={chart.title} size="lg">
        <DialogBody>
          <Stack gap={4}>
            <ChartSummary chart={chart} />
            {open && <LineChart {...line} height={320} />}
          </Stack>
        </DialogBody>
      </Dialog>
    </Card>
  );
}

/** Bốn con số của cả giờ qua: hiện tại, trung bình, cao nhất, thấp nhất. */
function ChartSummary({ chart }: { chart: ChartSpec }) {
  const { t } = useTranslation();
  const stats = summarize(chart.points);
  const show = (value: number | undefined) => (value === undefined ? "—" : chart.formatValue(value));

  return (
    <StatGrid>
      <StatTile label={t("overview.now")} value={show(stats?.last)} />
      <StatTile label={t("overview.average")} value={show(stats?.average)} />
      <StatTile label={t("overview.peak")} value={show(stats?.max)} />
      <StatTile label={t("overview.lowest")} value={show(stats?.min)} />
    </StatGrid>
  );
}
