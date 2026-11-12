import { useMutation } from "@tanstack/react-query";
import { Download, Lock, LockOpen, Pencil, RotateCcw, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { backupsApi, type Backup } from "../../../api/backups";
import { errorMessage, type Server } from "../../../api/client";
import { ConfirmDialog, Notice, type RowAction } from "../../../ui";
import { NoteDialog } from "./NoteDialog";

export function formatTime(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString();
}

/**
 * Thao tác trên từng bản: khôi phục (phải tích "Tôi hiểu"), tải xuống, sửa ghi chú, bảo vệ, xóa.
 * Trả về menu cho bảng, các hộp thoại, Job khôi phục đang chạy và lỗi của thao tác nhanh.
 */
export function useBackupActions(server: Server, refresh: () => void) {
  const { t } = useTranslation();
  const [restoring, setRestoring] = useState<Backup | null>(null);
  const [deleting, setDeleting] = useState<Backup | null>(null);
  const [editing, setEditing] = useState<Backup | null>(null);
  const [restoreJob, setRestoreJob] = useState<number | null>(null);
  const restore = useMutation({
    mutationFn: (b: Backup) => backupsApi.restore(server.id, b.id),
    onSuccess: (started) => {
      setRestoring(null);
      setRestoreJob(started.job_id);
    },
  });
  const remove = useMutation({
    mutationFn: (b: Backup) => backupsApi.remove(server.id, b.id),
    onSuccess: () => {
      setDeleting(null);
      refresh();
    },
  });
  const protect = useMutation({
    mutationFn: (b: Backup) => backupsApi.update(server.id, b.id, { protected: !b.protected }),
    onSuccess: refresh,
  });
  const actions: RowAction<Backup>[] = [
    { label: t("backups.restore"), icon: RotateCcw, onSelect: setRestoring },
    { label: t("backups.download"), icon: Download, onSelect: (b) => window.location.assign(backupsApi.downloadUrl(server.id, b.id)) },
    { label: t("backups.editNote"), icon: Pencil, onSelect: setEditing },
    { label: t("backups.protect"), icon: Lock, onSelect: (b) => protect.mutate(b), hidden: (b) => b.protected },
    { label: t("backups.unprotect"), icon: LockOpen, onSelect: (b) => protect.mutate(b), hidden: (b) => !b.protected },
    { label: t("backups.delete"), icon: Trash2, tone: "danger", onSelect: setDeleting, disabled: (b) => b.protected },
  ];
  const running = server.state !== "stopped" && server.state !== "crashed";

  const dialogs = (
    <>
      <ConfirmDialog
        open={restoring !== null}
        title={t("backups.restoreTitle")}
        confirmLabel={t("backups.restore")}
        requireAck={t("backups.understand")}
        loading={restore.isPending}
        onCancel={() => setRestoring(null)}
        onConfirm={() => restoring && restore.mutate(restoring)}
      >
        <p>{t("backups.restoreBody", { time: restoring ? formatTime(restoring.created_at) : "" })}</p>
        {running && <p>{t("backups.restoreStops")}</p>}
        {restore.isError && <Notice tone="danger">{errorMessage(t, restore.error)}</Notice>}
      </ConfirmDialog>
      <ConfirmDialog
        open={deleting !== null}
        title={t("backups.deleteTitle")}
        confirmLabel={t("backups.delete")}
        loading={remove.isPending}
        onCancel={() => setDeleting(null)}
        onConfirm={() => deleting && remove.mutate(deleting)}
      >
        <p>{t("backups.deleteBody", { time: deleting ? formatTime(deleting.created_at) : "" })}</p>
      </ConfirmDialog>
      {editing && <NoteDialog serverId={server.id} backup={editing} onClose={() => setEditing(null)} onSaved={refresh} />}
    </>
  );
  return { actions, dialogs, restoreJob, failure: protect.error ?? remove.error };
}
