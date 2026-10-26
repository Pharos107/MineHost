import { useQuery } from "@tanstack/react-query";
import { Cpu, HardDrive, ListChecks, MemoryStick, Plus, Server as ServerIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Link, useNavigate } from "react-router";

import { api, type Server } from "../../api/client";
import { SERVERS_KEY } from "../../api/live";
import { formatBytes, formatMemoryMb } from "../../format";
import { Button, Card, DataTable, EmptyState, PageHeader, StatGrid, StatTile, StatusBadge, type Column } from "../../ui";
import { useJobsLive } from "../jobs/jobsLive";
import { serverName } from "../ServerListPage";
import { playAddress } from "../serverFilters";
import { homeStats } from "./stats";

/** Đủ dày để CPU trông "sống" mà không hỏi máy quá nhiều. */
const SYSTEM_REFRESH_MS = 5000;

/** Trang chủ (spec P2, mục 6.1): tình hình chung của máy và các server đang chạy. */
export function HomePage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const servers = useQuery({ queryKey: SERVERS_KEY, queryFn: api.servers });
  const goCreate = () => void navigate("/new");

  if (servers.isSuccess && servers.data.length === 0) {
    return (
      <>
        <PageHeader title={t("nav.home")} description={t("app.tagline")} />
        <EmptyState
          icon={ServerIcon}
          title={t("servers.emptyTitle")}
          description={t("servers.emptyDescription")}
          action={
            <Button variant="primary" icon={Plus} onClick={goCreate}>
              {t("servers.createFirst")}
            </Button>
          }
        />
      </>
    );
  }

  return (
    <>
      <PageHeader
        title={t("nav.home")}
        description={t("app.tagline")}
        actions={
          <Button variant="primary" icon={Plus} onClick={goCreate}>
            {t("servers.create")}
          </Button>
        }
      />
      <Stats servers={servers.data ?? []} />
      <RunningServers servers={servers.data ?? []} loading={servers.isPending} />
    </>
  );
}

function Stats({ servers }: { servers: Server[] }) {
  const { t, i18n } = useTranslation();
  const system = useQuery({ queryKey: ["system"], queryFn: api.system, refetchInterval: SYSTEM_REFRESH_MS });
  const jobs = useJobsLive();
  const stats = homeStats(servers, jobs.data ?? [], system.data);
  const number = new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 0 });
  // Máy rảnh thường dưới 1%, làm tròn về số nguyên thì luôn thấy 0.
  const percent = new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 1 });
  const bytes = (value: number) => formatBytes(value, i18n.language);

  return (
    <StatGrid>
      <StatTile icon={ServerIcon} label={t("home.running")} value={number.format(stats.running)} unit={`/ ${number.format(stats.total)}`} />
      <StatTile icon={ListChecks} label={t("home.jobs")} value={number.format(stats.jobsRunning)} />
      <StatTile icon={Cpu} label={t("home.cpu")} value={stats.cpuPercent === null ? "—" : percent.format(stats.cpuPercent)} unit="%" />
      <StatTile
        icon={MemoryStick}
        label={t("home.memory")}
        value={stats.memoryUsed === null ? "—" : bytes(stats.memoryUsed)}
        detail={stats.memoryTotal === null ? undefined : t("home.ofTotal", { total: bytes(stats.memoryTotal) })}
      />
      <StatTile icon={HardDrive} label={t("home.diskFree")} value={stats.diskFree === null ? "—" : bytes(stats.diskFree)} />
    </StatGrid>
  );
}

function RunningServers({ servers, loading }: { servers: Server[]; loading: boolean }) {
  const { t, i18n } = useTranslation();
  // Chỉ cần IP LAN; việc hỏi lại định kỳ để thẻ số lo, tránh hai lượt đo CPU sát nhau.
  const system = useQuery({ queryKey: ["system"], queryFn: api.system });
  const running = servers.filter((s) => s.state === "starting" || s.state === "running" || s.state === "stopping");
  const columns: Column<Server>[] = [
    { id: "name", header: t("servers.name"), cell: (s) => <Link to={`/servers/${s.id}`}>{serverName(s)}</Link> },
    { id: "state", header: t("servers.state"), cell: (s) => <StatusBadge state={s.state} />, width: "md" },
    { id: "address", header: t("servers.address"), cell: (s) => playAddress(s, system.data?.lan_addresses) ?? "—", width: "md" },
    {
      id: "memory",
      header: t("servers.memory"),
      cell: (s) => (s.memory_mb ? formatMemoryMb(s.memory_mb.max, i18n.language) : "—"),
      align: "end",
      width: "sm",
    },
  ];

  return (
    <Card title={t("home.runningServers")}>
      <DataTable
        label={t("home.runningServers")}
        columns={columns}
        rows={running}
        rowKey={(s) => s.id}
        rowLabel={serverName}
        loading={loading}
        empty={<p>{t("home.noneRunning")}</p>}
      />
    </Card>
  );
}
