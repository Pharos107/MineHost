import { Clock, Cpu, HardDrive, MemoryStick, Users } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { MetricSample, Server } from "../../../api/client";
import { formatBytes } from "../../../format";
import { Columns, Notice, Stack, StatGrid, StatTile } from "../../../ui";
import { usePlayers } from "../players/usePlayers";
import { useServer } from "../ServerLayout";
import { ConsoleTail } from "./ConsoleTail";
import { formatUptime } from "./duration";
import { useServerMetrics } from "./metricsLive";
import { OverviewCharts } from "./OverviewCharts";
import { ServerInfoCard } from "./ServerInfoCard";

const WITH_PROCESS: Server["state"][] = ["starting", "running", "stopping"];

/** Tổng quan một server (spec P2, mục 6.4): số hiện tại, biểu đồ 1 giờ gần nhất, thông tin và log cuối. */
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
      {online ? <OverviewCharts server={server} liveMetrics={metrics.data} now={now} /> : <Notice tone="info">{t("overview.offline")}</Notice>}
      <Columns count={2}>
        <ServerInfoCard server={server} />
        <ConsoleTail server={server} />
      </Columns>
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
        meter={last && maxMemory ? (last.memory_bytes / maxMemory) * 100 : undefined}
        detail={maxMemory ? t("overview.memoryOf", { max: bytes(maxMemory) }) : undefined}
      />
      <PlayersTile server={server} />
      <StatTile icon={Clock} label={t("overview.uptime")} value={uptime} />
      <StatTile icon={HardDrive} label={t("overview.disk")} value={disk === null ? "—" : bytes(disk)} detail={disk === null ? t("overview.diskPending") : undefined} />
    </StatGrid>
  );
}

/** Chỉ số người đang chơi trên tối đa; danh sách tên nằm ở trang Người chơi. */
function PlayersTile({ server }: { server: Server }) {
  const { t } = useTranslation();
  const players = usePlayers(server.id, server.state);
  const data = server.state === "running" ? players.data : undefined;
  const online = data ? data.players.filter((p) => p.online).length : null;

  return (
    <StatTile
      icon={Users}
      label={t("overview.players")}
      value={online === null ? "—" : String(online)}
      unit={data ? `/ ${data.max_players}` : undefined}
      detail={data ? t("overview.playersOnline") : undefined}
    />
  );
}
