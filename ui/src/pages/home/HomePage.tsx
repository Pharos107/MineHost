import { useQuery } from "@tanstack/react-query";
import type { TFunction } from "i18next";
import { Cpu, HardDrive, ListChecks, MemoryStick, Plus, Server as ServerIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import { api, type Server } from "../../api/client";
import { SERVERS_KEY } from "../../api/live";
import { formatBytes } from "../../format";
import { Button, Card, DataTable, EmptyState, PageHeader, ServerCardGrid, Spinner, Stack, StatStrip } from "../../ui";
import { useJobColumns } from "../jobs/JobsPage";
import { useJobsLive } from "../jobs/jobsLive";
import { ServerTile } from "../ServerTile";
import { homeStats, sortForHome } from "./stats";

/** Đủ dày để CPU trông "sống" mà không hỏi máy quá nhiều. */
const SYSTEM_REFRESH_MS = 5000;

/** Trang chủ (spec P2, mục 6.1): mỗi server một thẻ (cách vào, ai đang chơi), dưới là tình hình của máy. */
export function HomePage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const servers = useQuery({ queryKey: SERVERS_KEY, queryFn: api.servers });
  const system = useQuery({ queryKey: ["system"], queryFn: api.system, refetchInterval: SYSTEM_REFRESH_MS });
  const goCreate = () => void navigate("/new");
  const list = servers.data ?? [];

  if (servers.isSuccess && list.length === 0) {
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

  const stats = homeStats(list, [], system.data);
  return (
    <>
      <PageHeader
        title={t("nav.home")}
        description={servers.isSuccess ? summary(t, stats.running, stats.total) : undefined}
        actions={
          <Button variant="primary" icon={Plus} onClick={goCreate}>
            {t("servers.create")}
          </Button>
        }
      />
      <Stack gap={6}>
        {servers.isPending ? (
          <Spinner label={t("ui.loading")} />
        ) : (
          <ServerCardGrid>
            {sortForHome(list).map((server) => (
              <ServerTile key={server.id} server={server} lan={system.data?.lan_addresses} />
            ))}
          </ServerCardGrid>
        )}
        <Machine servers={list} />
        <RecentActivity />
      </Stack>
    </>
  );
}

function summary(t: TFunction, running: number, total: number): string {
  return running === 0 ? t("home.summaryNone") : t("home.summary", { running, total });
}

/** Dải số liệu của máy chạy MineHost: thông tin phụ, nên gọn một hàng. */
function Machine({ servers }: { servers: Server[] }) {
  const { t, i18n } = useTranslation();
  const system = useQuery({ queryKey: ["system"], queryFn: api.system, refetchInterval: SYSTEM_REFRESH_MS });
  const jobs = useJobsLive();
  const stats = homeStats(servers, jobs.data ?? [], system.data);
  // Máy rảnh thường dưới 1%, làm tròn về số nguyên thì luôn thấy 0.
  const percent = new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 1 });
  const bytes = (value: number | null) => (value === null ? "—" : formatBytes(value, i18n.language));
  const memory = stats.memoryUsed === null ? "—" : t("home.usedOfTotal", { used: bytes(stats.memoryUsed), total: bytes(stats.memoryTotal) });

  return (
    <StatStrip
      label={t("home.machine")}
      items={[
        { icon: Cpu, label: t("home.cpu"), value: stats.cpuPercent === null ? "—" : `${percent.format(stats.cpuPercent)} %` },
        { icon: MemoryStick, label: t("home.memory"), value: memory },
        { icon: HardDrive, label: t("home.diskFree"), value: bytes(stats.diskFree) },
        { icon: ListChecks, label: t("home.jobs"), value: String(stats.jobsRunning) },
      ]}
    />
  );
}

/** Số việc gần nhất hiện ở Trang chủ; xem hết ở trang Tác vụ. */
const RECENT_JOBS = 5;

/** Việc MineHost vừa làm (tạo, sao lưu, khôi phục…), để biết bản sao lưu đêm qua có chạy không. */
function RecentActivity() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const jobs = useJobsLive();
  const columns = useJobColumns().filter((column) => column.id !== "progress");
  const recent = (jobs.data ?? []).slice(0, RECENT_JOBS);
  if (jobs.isSuccess && recent.length === 0) return null;

  return (
    <Card
      title={t("home.recent")}
      actions={
        <Button variant="ghost" size="sm" icon={ListChecks} onClick={() => void navigate("/jobs")}>
          {t("home.allJobs")}
        </Button>
      }
    >
      <DataTable
        label={t("home.recent")}
        columns={columns}
        rows={recent}
        rowKey={(job) => String(job.id)}
        rowLabel={(job) => `#${job.id}`}
        loading={jobs.isPending}
        empty={null}
      />
    </Card>
  );
}
