import { Clock, Cpu, HardDrive, MemoryStick } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { MetricSample, Server } from "../../../api/client";
import { formatBytes } from "../../../format";
import { Card, LineChart, Notice, Stack, StatGrid, StatTile, type ChartPoint } from "../../../ui";
import { useServer } from "../ServerLayout";
import { formatUptime } from "./duration";
import { useServerMetrics } from "./metricsLive";

const HOUR_MS = 3_600_000;
const WITH_PROCESS: Server["state"][] = ["starting", "running", "stopping"];

/** Tổng quan một server (spec P2, mục 6.4): số hiện tại và biểu đồ 1 giờ gần nhất. */
export function OverviewPage() {
  const server = useServer();
  const { t } = useTranslation();
  const metrics = useServerMetrics(server.id, server.state);
  const online = WITH_PROCESS.includes(server.state);
  const samples = online ? (metrics.data?.samples ?? []) : [];
  // "Bây giờ" lấy từ dữ liệu (lần tải REST gần nhất hoặc mẫu mới nhất) để render không phụ thuộc
  // đồng hồ; mỗi mẫu mới đến 5 giây một lần là đủ để khung 1 giờ trượt theo.
  const lastAt = samples.at(-1)?.at;
  const now = Math.max(metrics.dataUpdatedAt, lastAt ? Date.parse(lastAt) : 0);

  return (
    <Stack gap={4}>
      <Tiles server={server} samples={samples} disk={metrics.data?.disk_bytes ?? null} since={metrics.data?.running_since ?? null} now={now} />
      {online ? <Charts server={server} samples={samples} now={now} /> : <Notice tone="info">{t("overview.offline")}</Notice>}
    </Stack>
  );
}

interface TilesProps {
  server: Server;
  samples: MetricSample[];
  disk: number | null;
  since: string | null;
  now: number;
}

function Tiles({ server, samples, disk, since, now }: TilesProps) {
  const { t, i18n } = useTranslation();
  const last = samples.at(-1);
  const bytes = (value: number) => formatBytes(value, i18n.language);
  const number = new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 1 });
  const maxMemory = server.memory_mb ? server.memory_mb.max * 1024 * 1024 : null;
  const uptime = since ? formatUptime(t, (now - Date.parse(since)) / 1000) : "—";

  return (
    <StatGrid>
      <StatTile icon={Cpu} label={t("overview.cpu")} value={last ? number.format(last.cpu_percent) : "—"} unit="%" detail={t("overview.cpuHint")} />
      <StatTile
        icon={MemoryStick}
        label={t("overview.memory")}
        value={last ? bytes(last.memory_bytes) : "—"}
        detail={maxMemory ? t("overview.memoryOf", { max: bytes(maxMemory) }) : undefined}
      />
      <StatTile icon={Clock} label={t("overview.uptime")} value={uptime} />
      <StatTile icon={HardDrive} label={t("overview.disk")} value={disk === null ? "—" : bytes(disk)} detail={disk === null ? t("overview.diskPending") : undefined} />
    </StatGrid>
  );
}

function Charts({ server, samples, now }: { server: Server; samples: MetricSample[]; now: number }) {
  const { t, i18n } = useTranslation();
  const end = now;
  const start = end - HOUR_MS;
  const time = new Intl.DateTimeFormat(i18n.language, { hour: "2-digit", minute: "2-digit" });
  const formatTime = (ms: number) => time.format(ms);
  const points = (pick: (s: MetricSample) => number): ChartPoint[] => samples.map((s) => ({ t: Date.parse(s.at), v: pick(s) }));
  const maxMemory = server.memory_mb ? server.memory_mb.max * 1024 * 1024 : undefined;

  return (
    <>
      <Card title={t("overview.cpuChart")}>
        <LineChart
          label={t("overview.cpuChart")}
          points={points((s) => s.cpu_percent)}
          start={start}
          end={end}
          yMax={100}
          formatValue={(v) => `${Math.round(v)} %`}
          formatTime={formatTime}
        />
      </Card>
      <Card title={t("overview.memoryChart")}>
        <LineChart
          label={t("overview.memoryChart")}
          points={points((s) => s.memory_bytes)}
          start={start}
          end={end}
          yMax={maxMemory}
          formatValue={(v) => formatBytes(v, i18n.language)}
          formatTime={formatTime}
        />
      </Card>
    </>
  );
}
