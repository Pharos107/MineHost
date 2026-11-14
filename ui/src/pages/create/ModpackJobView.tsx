import { useMutation } from "@tanstack/react-query";
import { ArrowLeft, ArrowRight, RotateCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import { api, ApiError, errorMessage, type Created } from "../../api/client";
import type { ModpackSummary } from "../../api/content";
import { useJob } from "../../api/live";
import { jobPercent, jobStepLabel } from "../../jobs";
import { Button, Card, Facts, Notice, PageHeader, ProgressBar, Stack } from "../../ui";

interface ModpackJobViewProps {
  created: Created;
  onRetry: () => void;
  onBack: () => void;
}

function parseSummary(raw: unknown): ModpackSummary {
  if (!raw || typeof raw !== "object") {
    return { installed_mods: 0, skipped_client: [], pending_curseforge: [] };
  }
  const obj = raw as Record<string, unknown>;
  return {
    installed_mods: typeof obj.installed_mods === "number" ? obj.installed_mods : 0,
    skipped_client: Array.isArray(obj.skipped_client) ? obj.skipped_client.filter((x): x is string => typeof x === "string") : [],
    pending_curseforge: Array.isArray(obj.pending_curseforge)
      ? obj.pending_curseforge.filter((x): x is string => typeof x === "string")
      : [],
  };
}

/** Tiến độ cài modpack từng file; xong thì hiện tóm tắt kết quả và nút mở trang server (spec P3, mục 6.2). */
export function ModpackJobView({ created, onRetry, onBack }: ModpackJobViewProps) {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const job = useJob(created.job_id);
  const cancel = useMutation({ mutationFn: () => api.cancelJob(created.job_id) });
  const state = job.data?.state;

  const back = (
    <Button variant="ghost" icon={ArrowLeft} onClick={onBack}>
      {t("wizard.back")}
    </Button>
  );

  let body;
  if (state === "failed") {
    const error = job.data?.error;
    body = (
      <Notice
        tone="danger"
        title={t("wizard.failedTitle")}
        action={
          <Stack direction="row" gap={2}>
            {back}
            <Button variant="primary" icon={RotateCw} onClick={onRetry}>
              {t("common.retry")}
            </Button>
          </Stack>
        }
      >
        {errorMessage(t, error ? new ApiError(error.code, error.params, 0) : undefined)}
      </Notice>
    );
  } else if (state === "cancelled") {
    body = <Notice title={t("wizard.cancelledTitle")} action={back} />;
  } else if (job.isError) {
    body = (
      <Notice tone="danger" action={back}>
        {errorMessage(t, job.error)}
      </Notice>
    );
  } else if (state === "succeeded") {
    const summary = parseSummary(job.data?.result);
    const items = [
      { label: t("modpack.summaryInstalled"), value: String(summary.installed_mods) },
      { label: t("modpack.summarySkippedClient"), value: String(summary.skipped_client.length) },
      ...(summary.pending_curseforge.length > 0
        ? [{ label: t("modpack.summaryPendingCf"), value: String(summary.pending_curseforge.length) }]
        : []),
    ];
    body = (
      <Stack gap={4}>
        <Notice tone="success" title={t("modpack.doneTitle")}>
          {t("modpack.doneDescription")}
        </Notice>
        <Facts items={items} />
        {summary.skipped_client.length > 0 && (
          <Notice tone="info" title={t("modpack.summarySkippedClient")}>
            {summary.skipped_client.join(", ")}
          </Notice>
        )}
        {summary.pending_curseforge.length > 0 && (
          <Notice tone="warning" title={t("modpack.pendingCfHint")}>
            {summary.pending_curseforge.join(", ")}
          </Notice>
        )}
        <div>
          <Button variant="primary" icon={ArrowRight} onClick={() => void navigate(`/servers/${created.server_id}`)}>
            {t("modpack.openServer")}
          </Button>
        </div>
      </Stack>
    );
  } else {
    const progress = job.data?.progress;
    body = (
      <Stack gap={4}>
        <ProgressBar
          label={progress ? jobStepLabel(t, progress, i18n.language) : t("jobs.starting")}
          value={progress ? jobPercent(progress) : undefined}
        />
        {cancel.isError && <Notice tone="danger">{errorMessage(t, cancel.error)}</Notice>}
        <div>
          <Button variant="ghost" loading={cancel.isPending} onClick={() => cancel.mutate()}>
            {t("ui.cancel")}
          </Button>
        </div>
      </Stack>
    );
  }

  return (
    <>
      <PageHeader title={t("modpack.creatingTitle")} description={t("modpack.creatingDescription")} />
      <Card>{body}</Card>
    </>
  );
}
