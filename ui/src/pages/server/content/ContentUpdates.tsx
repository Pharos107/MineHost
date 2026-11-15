import { useMutation } from "@tanstack/react-query";
import { RefreshCw } from "lucide-react";
import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage, type Job } from "../../../api/client";
import { contentApi, type ContentKind, type UpdateCandidate, type UpdateReport } from "../../../api/content";
import { Button, ConfirmDialog, Notice, Stack } from "../../../ui";
import { JobProgress, jobError } from "../JobProgress";
import { fileNameOf, updateItems } from "./content";

interface ContentUpdatesProps {
  serverId: string;
  kind: ContentKind;
  report: UpdateReport | null;
  onReport: (report: UpdateReport | null) => void;
  /** Mục đang chờ người dùng xác nhận cập nhật; `null` là không mở hộp thoại. */
  confirming: UpdateCandidate[] | null;
  onConfirming: (items: UpdateCandidate[] | null) => void;
  onUpdated: () => void;
}

/**
 * Nút "Kiểm tra cập nhật" và hộp thoại cập nhật (spec P3, mục 3.9, 6.3). Cập nhật tắt server nếu
 * đang chạy và luôn sao lưu trước; lỗi giữa chừng chỉ người dùng tới bản sao lưu đó.
 */
export function ContentUpdates({ serverId, kind, report, onReport, confirming, onConfirming, onUpdated }: ContentUpdatesProps) {
  const { t } = useTranslation();
  const [checkJob, setCheckJob] = useState<number | null>(null);
  const [updateJob, setUpdateJob] = useState<number | null>(null);
  const [failure, setFailure] = useState<{ error: unknown; backup: boolean } | null>(null);
  const [updated, setUpdated] = useState<number | null>(null);

  const check = useMutation({
    mutationFn: () => contentApi.checkUpdates(serverId),
    onSuccess: (res) => {
      setUpdated(null);
      setFailure(null);
      setCheckJob(res.job_id);
    },
  });
  const apply = useMutation({
    mutationFn: (items: UpdateCandidate[]) => contentApi.applyUpdates(serverId, updateItems(items)),
    onSuccess: (res) => setUpdateJob(res.job_id),
  });

  const onChecked = useCallback(
    (job: Job) => {
      setCheckJob(null);
      const error = jobError(job);
      if (error) setFailure({ error, backup: false });
      else onReport((job.result ?? null) as unknown as UpdateReport | null);
    },
    [onReport],
  );
  const onApplied = useCallback(
    (job: Job) => {
      setUpdateJob(null);
      const count = confirming?.length ?? 0;
      onConfirming(null);
      const error = jobError(job);
      if (error) {
        setFailure({ error, backup: typeof error.params.backup_id === "string" });
      } else if (job.state === "succeeded") {
        setUpdated(count);
        onReport(null);
      }
      onUpdated();
    },
    [confirming, onConfirming, onReport, onUpdated],
  );

  const updates = report?.updates ?? [];
  const running = updateJob !== null || apply.isPending;
  return (
    <Stack gap={3}>
      <Stack direction="row" gap={2} wrap>
        <Button icon={RefreshCw} loading={checkJob !== null || check.isPending} onClick={() => check.mutate()}>
          {t("content.updates.check")}
        </Button>
      </Stack>
      {checkJob !== null && <JobProgress jobId={checkJob} onFinish={onChecked} />}
      {check.isError && <Notice tone="danger">{errorMessage(t, check.error)}</Notice>}
      {report && updates.length === 0 && (
        <Notice tone="success">{t(`content.${kind}.upToDate`, { unknown: report.unknown })}</Notice>
      )}
      {updates.length > 0 && (
        <Notice
          tone="info"
          action={
            <Button size="sm" variant="primary" onClick={() => onConfirming(updates)}>
              {t("content.updates.updateAll", { count: updates.length })}
            </Button>
          }
        >
          {t("content.updates.found", { count: updates.length })}
        </Notice>
      )}
      {updated !== null && <Notice tone="success">{t("content.updates.done", { count: updated })}</Notice>}
      {failure && (
        <Notice tone="danger" title={failure.backup ? t("content.updates.failedWithBackup") : undefined}>
          {errorMessage(t, failure.error)}
        </Notice>
      )}
      <ConfirmDialog
        open={confirming !== null}
        title={t("content.updates.confirmTitle", { count: confirming?.length ?? 0 })}
        confirmLabel={t("content.updates.confirm")}
        tone="primary"
        loading={running}
        onCancel={() => {
          if (!running) onConfirming(null);
        }}
        onConfirm={() => confirming && apply.mutate(confirming)}
      >
        <p>{t("content.updates.confirmBody")}</p>
        <ul>
          {(confirming ?? []).map((item) => (
            <li key={item.file}>
              {t("content.updates.item", { file: fileNameOf(item.file), version: item.latest_version_name })}
            </li>
          ))}
        </ul>
        {updateJob !== null && <JobProgress jobId={updateJob} onFinish={onApplied} />}
        {apply.isError && <Notice tone="danger">{errorMessage(t, apply.error)}</Notice>}
      </ConfirmDialog>
    </Stack>
  );
}
