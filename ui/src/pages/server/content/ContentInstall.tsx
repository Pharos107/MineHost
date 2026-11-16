// Phần dùng chung khi cài mod: kế hoạch cài và tiến độ Job.
import { useMutation } from "@tanstack/react-query";
import { ArrowLeft, X } from "lucide-react";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { api, ApiError, errorMessage } from "../../../api/client";
import type { ContentKind, ContentPlan, PlannedContent } from "../../../api/content";
import { useJob } from "../../../api/live";
import { formatBytes } from "../../../format";
import { jobPercent, jobStepLabel } from "../../../jobs";
import { Badge, Button, Card, DialogBody, DialogFooter, Facts, IconButton, Notice, ProgressBar, Stack } from "../../../ui";

interface PlanDetailsProps {
  plan: ContentPlan;
  kind: ContentKind;
  /** Có thì mỗi mục chính có nút bỏ khỏi danh sách chờ. */
  onRemove?: (item: PlannedContent) => void;
}

/** Danh sách sẽ cài: mục chính và dependency, kèm nhãn chỉ client, tải tay, thay bản cũ. */
export function PlanDetails({ plan, kind, onRemove }: PlanDetailsProps) {
  const { t, i18n } = useTranslation();
  const mainItems = plan.items.filter((item) => !item.is_dependency);
  const depItems = plan.items.filter((item) => item.is_dependency);
  const hasClientOnly = plan.items.some((item) => item.client_only);
  const hasManualDownload = plan.items.some((item) => item.manual_download);

  const renderItem = (item: PlannedContent) => (
    <Stack direction="row" gap={2} align="center" wrap key={item.project_id}>
      <strong>{item.project_name}</strong>
      <Badge>{item.version_number}</Badge>
      <span>{formatBytes(item.file_size, i18n.language)}</span>
      {item.client_only && <Badge tone="warning">{t("content.clientOnly")}</Badge>}
      {item.manual_download && <Badge tone="warning">{t("content.addDialog.manualDownload")}</Badge>}
      {item.replaces_existing && <Badge tone="info">{t("content.addDialog.replacesExisting")}</Badge>}
      {item.manual_download && item.download_page_url && (
        <a href={item.download_page_url} target="_blank" rel="noreferrer">
          {t("content.addDialog.openDownloadPage")}
        </a>
      )}
      {onRemove && !item.is_dependency && (
        <IconButton icon={X} size="sm" label={t("catalog.queue.remove", { name: item.project_name })} onClick={() => onRemove(item)} />
      )}
    </Stack>
  );

  return (
    <Stack gap={3}>
      <Card title={t("content.addDialog.planTitle")}>
        <Facts
          items={[
            { label: t(`content.${kind}.mainItem`), value: <Stack gap={2}>{mainItems.map(renderItem)}</Stack> },
            ...(depItems.length > 0
              ? [{ label: t("content.addDialog.dependencies", { count: depItems.length }), value: <Stack gap={2}>{depItems.map(renderItem)}</Stack> }]
              : []),
          ]}
        />
      </Card>
      {hasClientOnly && <Notice tone="info">{t("content.addDialog.clientOnlyHint")}</Notice>}
      {hasManualDownload && <Notice tone="warning">{t("content.addDialog.manualDownloadHint")}</Notice>}
    </Stack>
  );
}

export interface InstallJobStepProps {
  jobId: number;
  onBack: () => void;
  onClose: () => void;
  onSucceeded: () => void;
}

/** Tiến độ Job cài trong hộp thoại; lỗi thì cho quay lại. */
export function InstallJobStep({ jobId, onBack, onClose, onSucceeded }: InstallJobStepProps) {
  const { t, i18n } = useTranslation();
  const job = useJob(jobId);
  const cancel = useMutation({ mutationFn: () => api.cancelJob(jobId) });
  const doneRef = useRef(false);
  const state = job.data?.state;

  useEffect(() => {
    if (state === "succeeded" && !doneRef.current) {
      doneRef.current = true;
      onSucceeded();
    }
  }, [state, onSucceeded]);

  if (state === "failed" || state === "cancelled" || job.isError) {
    const error = job.data?.error;
    const notice = job.isError ? (
      <Notice tone="danger">{errorMessage(t, job.error)}</Notice>
    ) : state === "failed" ? (
      <Notice tone="danger" title={t("content.addDialog.failedTitle")}>
        {errorMessage(t, error ? new ApiError(error.code, error.params, 0) : undefined)}
      </Notice>
    ) : (
      <Notice>{t("content.addDialog.cancelledTitle")}</Notice>
    );
    return (
      <>
        <DialogBody>{notice}</DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {t("ui.close")}
          </Button>
          <Button variant={state === "failed" ? "primary" : "secondary"} icon={ArrowLeft} onClick={onBack}>
            {t("content.addDialog.back")}
          </Button>
        </DialogFooter>
      </>
    );
  }

  const progress = job.data?.progress;
  return (
    <>
      <DialogBody>
        <ProgressBar
          label={progress ? jobStepLabel(t, progress, i18n.language) : t("jobs.starting")}
          value={progress ? jobPercent(progress) : undefined}
        />
        {cancel.isError && <Notice tone="danger">{errorMessage(t, cancel.error)}</Notice>}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" loading={cancel.isPending} onClick={() => cancel.mutate()}>
          {t("ui.cancel")}
        </Button>
      </DialogFooter>
    </>
  );
}
