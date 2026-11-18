import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { RotateCcw, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "../../../api/client";
import { filesApi, type TrashBatch } from "../../../api/files";
import { formatBytes } from "../../../format";
import { Button, Card, ConfirmDialog, Dialog, DialogBody, DialogFooter, EmptyState, IconButton, Notice, Spinner, Stack, toast } from "../../../ui";

interface TrashDialogProps {
  serverId: string;
  onClose: () => void;
  onChanged: () => void;
}

/** Xem các đợt file đã chuyển vào `trash/` của server, khôi phục hoặc dọn sạch. */
export function TrashDialog({ serverId, onClose, onChanged }: TrashDialogProps) {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const [confirmEmpty, setConfirmEmpty] = useState(false);
  const trash = useQuery({ queryKey: ["files-trash", serverId], queryFn: () => filesApi.trashList(serverId) });
  const time = new Intl.DateTimeFormat(i18n.language, { dateStyle: "short", timeStyle: "short" });

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["files-trash", serverId] });
    onChanged();
  };

  const restore = useMutation({
    mutationFn: (batch: string) => filesApi.trashRestore(serverId, batch),
    onSuccess: (items) => {
      toast.show({ tone: "success", title: t("files.trash.restored", { count: items.length }) });
      refresh();
    },
  });

  const empty = useMutation({
    mutationFn: (batch?: string) => filesApi.trashEmpty(serverId, batch),
    onSuccess: (_data, batch) => {
      setConfirmEmpty(false);
      toast.show({ tone: "success", title: batch ? t("files.trash.deletedBatch") : t("files.trash.emptied") });
      refresh();
    },
  });

  const batches = trash.data ?? [];
  const activeError = restore.error ?? empty.error ?? trash.error;

  return (
    <>
      <Dialog open onClose={onClose} title={t("files.trash.title")} size="lg">
        <DialogBody>
          <Stack gap={3}>
            {activeError && <Notice tone="danger">{errorMessage(t, activeError)}</Notice>}
            {trash.isPending && <Spinner label={t("ui.loading")} />}
            {trash.isSuccess && batches.length === 0 && <EmptyState icon={Trash2} title={t("files.trash.empty")} />}
            {batches.map((batch) => (
              <TrashBatchCard
                key={batch.id}
                batch={batch}
                timeLabel={batch.deleted_at ? time.format(new Date(batch.deleted_at)) : batch.id}
                busy={restore.isPending || empty.isPending}
                onRestore={() => restore.mutate(batch.id)}
                onDelete={() => empty.mutate(batch.id)}
              />
            ))}
          </Stack>
        </DialogBody>
        <DialogFooter>
          {batches.length > 0 && (
            <Button variant="danger" icon={Trash2} loading={empty.isPending} onClick={() => setConfirmEmpty(true)}>
              {t("files.trash.emptyAll")}
            </Button>
          )}
          <Button variant="ghost" onClick={onClose}>
            {t("ui.close")}
          </Button>
        </DialogFooter>
      </Dialog>
      <ConfirmDialog
        open={confirmEmpty}
        title={t("files.trash.emptyConfirm")}
        confirmLabel={t("files.trash.emptyAll")}
        loading={empty.isPending}
        onCancel={() => setConfirmEmpty(false)}
        onConfirm={() => empty.mutate(undefined)}
      >
        <p>{t("files.trash.emptyBody")}</p>
      </ConfirmDialog>
    </>
  );
}

function TrashBatchCard({
  batch,
  timeLabel,
  busy,
  onRestore,
  onDelete,
}: {
  batch: TrashBatch;
  timeLabel: string;
  busy: boolean;
  onRestore: () => void;
  onDelete: () => void;
}) {
  const { t, i18n } = useTranslation();
  const preview = batch.items.slice(0, 3).join(", ");
  const extra = batch.items.length > 3 ? ` (+${batch.items.length - 3})` : "";
  return (
    <Card>
      <Stack direction="row" gap={3} align="center" justify="between" wrap>
        <Stack gap={1}>
          <strong>{timeLabel}</strong>
          <span>
            {t("files.trash.itemsCount", { count: batch.items.length })} · {formatBytes(batch.bytes, i18n.language)}
          </span>
          <code>
            {preview}
            {extra}
          </code>
        </Stack>
        <Stack direction="row" gap={2} align="center">
          <Button size="sm" icon={RotateCcw} disabled={busy} onClick={onRestore}>
            {t("files.trash.restore")}
          </Button>
          <IconButton icon={Trash2} label={t("files.trash.deleteBatch")} size="sm" tone="danger" disabled={busy} onClick={onDelete} />
        </Stack>
      </Stack>
    </Card>
  );
}
