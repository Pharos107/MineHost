import { useMutation, useQuery } from "@tanstack/react-query";
import { ListChecks, RotateCw, X } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";

import { api, ApiError, errorMessage, type Job } from "../../api/client";
import { SERVERS_KEY } from "../../api/live";
import { jobPercent, jobStepLabel } from "../../jobs";
import {
  Badge,
  Button,
  DataTable,
  EmptyState,
  Notice,
  PageHeader,
  ProgressBar,
  Select,
  type BadgeTone,
  type Column,
  type RowAction,
} from "../../ui";
import { serverName } from "../ServerListPage";
import { useJobsLive } from "./jobsLive";

const STATE_TONE: Record<Job["state"], BadgeTone> = {
  running: "info",
  succeeded: "success",
  failed: "danger",
  cancelled: "neutral",
};

/** Trang Tác vụ (spec P2, mục 6.3): mọi Job MineHost đang giữ, cập nhật realtime, hủy được. */
export function JobsPage() {
  const { t } = useTranslation();
  const jobs = useJobsLive();
  const [serverFilter, setServerFilter] = useState("");
  const cancel = useMutation({ mutationFn: (id: number) => api.cancelJob(id) });
  const columns = useJobColumns();
  const rows = (jobs.data ?? []).filter((job) => serverFilter === "" || job.server_id === serverFilter);
  const actions: RowAction<Job>[] = [
    { label: t("jobsPage.cancel"), icon: X, tone: "danger", onSelect: (job) => cancel.mutate(job.id), hidden: (job) => job.state !== "running" },
  ];

  return (
    <>
      <PageHeader title={t("nav.jobs")} description={t("jobsPage.description")} />
      {cancel.isError && <Notice tone="danger">{errorMessage(t, cancel.error)}</Notice>}
      {jobs.isError ? (
        <Notice
          tone="danger"
          action={
            <Button size="sm" icon={RotateCw} onClick={() => void jobs.refetch()}>
              {t("common.retry")}
            </Button>
          }
        >
          {errorMessage(t, jobs.error)}
        </Notice>
      ) : (
        <DataTable
          label={t("nav.jobs")}
          columns={columns}
          rows={rows}
          rowKey={(job) => String(job.id)}
          rowLabel={(job) => `#${job.id}`}
          actions={actions}
          inlineActions={1}
          toolbar={(jobs.data ?? []).length > 0 && <ServerSelect value={serverFilter} onChange={setServerFilter} />}
          loading={jobs.isPending}
          empty={<EmptyState icon={ListChecks} title={t("jobsPage.empty")} description={t("jobsPage.emptyHint")} />}
        />
      )}
    </>
  );
}

function ServerSelect({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const { t } = useTranslation();
  const servers = useQuery({ queryKey: SERVERS_KEY, queryFn: api.servers });
  return (
    <Select aria-label={t("jobsPage.filterServer")} value={value} onChange={(e) => onChange(e.target.value)}>
      <option value="">{t("jobsPage.allServers")}</option>
      {(servers.data ?? []).map((server) => (
        <option key={server.id} value={server.id}>
          {serverName(server)}
        </option>
      ))}
    </Select>
  );
}

/** Cột của bảng Job; Trang chủ dùng lại cho mục "Hoạt động gần đây". */
export function useJobColumns(): Column<Job>[] {
  const { t, i18n } = useTranslation();
  const servers = useQuery({ queryKey: SERVERS_KEY, queryFn: api.servers });
  const nameOf = (id: string) => {
    const server = servers.data?.find((s) => s.id === id);
    return server ? serverName(server) : id;
  };
  const time = new Intl.DateTimeFormat(i18n.language, { dateStyle: "short", timeStyle: "short" });

  return [
    { id: "kind", header: t("jobsPage.kind"), cell: (job) => t(`jobsPage.kinds.${job.kind}`, { defaultValue: job.kind }) },
    {
      id: "server",
      header: t("jobsPage.server"),
      cell: (job) => (job.server_id ? <Link to={`/servers/${job.server_id}`}>{nameOf(job.server_id)}</Link> : "—"),
    },
    {
      id: "state",
      header: t("jobsPage.state"),
      cell: (job) => (
        <Badge tone={STATE_TONE[job.state]} dot={job.state === "running" ? "pulse" : true}>
          {t(`jobsPage.states.${job.state}`)}
        </Badge>
      ),
      width: "md",
    },
    { id: "progress", header: t("jobsPage.progress"), cell: (job) => <JobDetail job={job} /> },
    {
      id: "started",
      header: t("jobsPage.started"),
      cell: (job) => time.format(new Date(job.started_at)),
      sortValue: (job) => job.id,
      width: "md",
    },
  ];
}

/** Đang chạy thì hiện bước và thanh tiến độ; lỗi thì hiện câu đã dịch theo mã lỗi. */
function JobDetail({ job }: { job: Job }) {
  const { t, i18n } = useTranslation();
  if (job.state === "running") {
    return job.progress ? (
      <ProgressBar label={jobStepLabel(t, job.progress, i18n.language)} value={jobPercent(job.progress)} />
    ) : (
      t("jobs.starting")
    );
  }
  if (job.state === "failed" && job.error) {
    return errorMessage(t, new ApiError(job.error.code, job.error.params, 0));
  }
  return "—";
}
