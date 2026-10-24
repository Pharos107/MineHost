import { useTranslation } from "react-i18next";

import type { Server } from "../../api/client";
import { useJob } from "../../api/live";
import { jobPercent, jobStepLabel } from "../../jobs";
import { Card, Notice, ProgressBar } from "../../ui";

/** Thông báo theo tình trạng server: đang cài, hỏng, crash, chờ tự bật lại, bật quá lâu. */
export function ServerNotices({ server }: { server: Server }) {
  const { t } = useTranslation();

  if (server.state === "installing" && server.job_id != null) return <InstallProgress jobId={server.job_id} />;
  if (server.state === "broken") {
    const reason = (server.broken ?? {}) as { kind?: string };
    return (
      <Notice tone="danger" title={t("server.broken.title")}>
        {t(`server.broken.${reason.kind ?? ""}`, { ...reason, defaultValue: t("server.broken.unknown") })}
      </Notice>
    );
  }
  const hint = server.crash?.hint;
  return (
    <>
      {server.state === "crashed" && server.crash && (
        <Notice tone="danger" title={t("server.crashed")}>
          {t(`crashHints.${hint ?? ""}`, { defaultValue: t("crashHints.unknown") })}
        </Notice>
      )}
      {server.restart_pending && <Notice tone="info">{t("server.restartPending")}</Notice>}
      {server.slow_start && server.state === "starting" && <Notice tone="warning">{t("server.slowStart")}</Notice>}
    </>
  );
}

function InstallProgress({ jobId }: { jobId: number }) {
  const { t, i18n } = useTranslation();
  const progress = useJob(jobId).data?.progress;
  return (
    <Card>
      <ProgressBar
        label={progress ? jobStepLabel(t, progress, i18n.language) : t("jobs.starting")}
        value={progress ? jobPercent(progress) : undefined}
      />
    </Card>
  );
}
