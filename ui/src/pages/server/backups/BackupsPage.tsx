import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Archive, Lock } from "lucide-react";
import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import { backupsApi, backupsKey, type Backup } from "../../../api/backups";
import { errorMessage } from "../../../api/client";
import { formatBytes } from "../../../format";
import { Badge, DataTable, EmptyState, Notice, Stack, type BadgeTone, type Column } from "../../../ui";
import { useServer } from "../ServerLayout";
import { BackupNowCard, ScheduleCard } from "./BackupCards";
import { JobLine } from "./JobLine";
import { formatTime, useBackupActions } from "./useBackupActions";

const KIND_TONE: Record<string, BadgeTone> = { manual: "info", scheduled: "neutral", before_restore: "warning" };

/** Trang Sao lưu (spec P2, mục 6.8): sao lưu ngay, lịch tự động, bảng các bản và thao tác từng bản. */
export function BackupsPage() {
  const { t } = useTranslation();
  const server = useServer();
  const queryClient = useQueryClient();
  const list = useQuery({ queryKey: backupsKey(server.id), queryFn: () => backupsApi.list(server.id) });
  const refresh = useCallback(() => void queryClient.invalidateQueries({ queryKey: backupsKey(server.id) }), [queryClient, server.id]);
  const { actions, dialogs, restoreJob, failure } = useBackupActions(server, refresh);
  const columns = useColumns();

  return (
    <Stack gap={5}>
      <BackupNowCard server={server} />
      {/* Đổi lịch ở tab khác thì thẻ đọc lại giá trị mới. */}
      <ScheduleCard key={`${server.backup?.schedule}-${server.backup?.keep}`} server={server} />
      {restoreJob !== null && <JobLine key={restoreJob} jobId={restoreJob} onDone={refresh} />}
      {failure && <Notice tone="danger">{errorMessage(t, failure)}</Notice>}
      {list.isError ? (
        <Notice tone="danger">{errorMessage(t, list.error)}</Notice>
      ) : (
        <DataTable
          label={t("backups.table")}
          columns={columns}
          rows={list.data ?? []}
          rowKey={(b) => b.id}
          rowLabel={(b) => formatTime(b.created_at)}
          actions={actions}
          inlineActions={1}
          searchText={(b) => `${b.note ?? ""} ${b.minecraft}`}
          loading={list.isPending}
          empty={<EmptyState icon={Archive} title={t("backups.empty")} description={t("backups.emptyHint")} />}
        />
      )}
      {dialogs}
    </Stack>
  );
}

function useColumns(): Column<Backup>[] {
  const { t, i18n } = useTranslation();
  return [
    { id: "time", header: t("backups.when"), cell: (b) => formatTime(b.created_at), sortValue: (b) => b.created_at },
    { id: "kind", header: t("backups.kind"), cell: (b) => <Badge tone={KIND_TONE[b.kind] ?? "neutral"}>{t(`backups.kinds.${b.kind}`)}</Badge>, width: "sm" },
    { id: "size", header: t("backups.size"), cell: (b) => formatBytes(b.size, i18n.language), sortValue: (b) => b.size, align: "end", width: "sm" },
    { id: "version", header: t("backups.version"), cell: (b) => b.minecraft, width: "sm" },
    { id: "note", header: t("backups.note"), cell: (b) => b.note ?? "—" },
    {
      id: "protected",
      header: t("backups.protected"),
      cell: (b) => (b.protected ? <Lock size={16} aria-label={t("backups.protected")} /> : null),
      align: "center",
      width: "xs",
    },
  ];
}
