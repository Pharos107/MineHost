import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { ApiError, errorMessage } from "../../../api/client";
import { useJob } from "../../../api/live";
import { jobPercent, jobStepLabel } from "../../../jobs";
import { Notice, ProgressBar } from "../../../ui";

/** Tiến độ một Job sao lưu hoặc khôi phục. Xong thì gọi `onDone` một lần; lỗi thì báo lỗi đã dịch. */
export function JobLine({ jobId, onDone }: { jobId: number; onDone: () => void }) {
  const { t, i18n } = useTranslation();
  const job = useJob(jobId);
  const state = job.data?.state;

  useEffect(() => {
    if (state === "succeeded") onDone();
  }, [state, onDone]);

  if (state === "failed") {
    const error = job.data?.error;
    return <Notice tone="danger">{errorMessage(t, error ? new ApiError(error.code, error.params, 0) : undefined)}</Notice>;
  }
  if (state === "cancelled") return <Notice>{t("backups.cancelled")}</Notice>;
  if (state === "succeeded") return null;
  const progress = job.data?.progress;
  return (
    <ProgressBar
      label={progress ? jobStepLabel(t, progress, i18n.language) : t("jobs.starting")}
      value={progress ? jobPercent(progress) : undefined}
    />
  );
}
