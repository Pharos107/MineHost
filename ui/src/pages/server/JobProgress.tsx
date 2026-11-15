import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { ApiError, type Job } from "../../api/client";
import { useJob } from "../../api/live";
import { jobPercent, jobStepLabel } from "../../jobs";
import { ProgressBar } from "../../ui";

/** Thanh tiến độ của một Job; gọi `onFinish` đúng một lần khi Job rời trạng thái `running`. */
export function JobProgress({ jobId, onFinish }: { jobId: number; onFinish: (job: Job) => void }) {
  const { t, i18n } = useTranslation();
  const job = useJob(jobId);
  const done = useRef(false);
  const data = job.data;
  useEffect(() => {
    if (data && data.state !== "running" && !done.current) {
      done.current = true;
      onFinish(data);
    }
  }, [data, onFinish]);
  const progress = data?.progress;
  return (
    <ProgressBar
      label={progress ? jobStepLabel(t, progress, i18n.language) : t("jobs.starting")}
      value={progress ? jobPercent(progress) : undefined}
    />
  );
}

/** Lỗi của Job đã thất bại, dạng `ApiError` để dùng chung `errorMessage`. */
export function jobError(job: Job): ApiError | null {
  if (job.state !== "failed" || !job.error) return null;
  return new ApiError(job.error.code, job.error.params as Record<string, unknown>, 0);
}
