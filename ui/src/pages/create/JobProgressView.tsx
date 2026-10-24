import { useMutation } from "@tanstack/react-query";
import { ArrowLeft, RotateCw } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import { api, ApiError, errorMessage, type Created } from "../../api/client";
import { useJob } from "../../api/live";
import { jobPercent, jobStepLabel } from "../../jobs";
import { Button, Card, Notice, PageHeader, ProgressBar, Stack } from "../../ui";

interface JobProgressViewProps {
  created: Created;
  /** Gửi lại đúng yêu cầu tạo server vừa rồi. */
  onRetry: () => void;
  /** Về bước cuối của wizard, giữ nguyên những gì đã nhập. */
  onBack: () => void;
}

/** Tiến độ Job tạo server; xong thì chuyển sang trang chi tiết, nơi có nút "Bật server". */
export function JobProgressView({ created, onRetry, onBack }: JobProgressViewProps) {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const job = useJob(created.job_id);
  const cancel = useMutation({ mutationFn: () => api.cancelJob(created.job_id) });
  const state = job.data?.state;

  useEffect(() => {
    if (state === "succeeded") void navigate(`/servers/${created.server_id}`, { replace: true });
  }, [state, navigate, created.server_id]);

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
    body = <Notice tone="danger" action={back}>{errorMessage(t, job.error)}</Notice>;
  } else {
    const progress = job.data?.progress;
    body = (
      <>
        <ProgressBar
          label={progress ? jobStepLabel(t, progress, i18n.language) : t("jobs.starting")}
          value={progress ? jobPercent(progress) : undefined}
          tone={state === "succeeded" ? "success" : "primary"}
        />
        {cancel.isError && <Notice tone="danger">{errorMessage(t, cancel.error)}</Notice>}
        <div>
          <Button variant="ghost" loading={cancel.isPending} disabled={state === "succeeded"} onClick={() => cancel.mutate()}>
            {t("ui.cancel")}
          </Button>
        </div>
      </>
    );
  }

  return (
    <>
      <PageHeader title={t("wizard.creatingTitle")} description={t("wizard.creatingDescription")} />
      <Card>{body}</Card>
    </>
  );
}
