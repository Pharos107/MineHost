import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { backupsApi, type Backup } from "../../../api/backups";
import { errorMessage } from "../../../api/client";
import { Button, Dialog, DialogBody, DialogFooter, Field, Notice, TextInput } from "../../../ui";

/** Sửa ghi chú một bản sao lưu; để trống là xóa ghi chú. */
export function NoteDialog({ serverId, backup, onClose, onSaved }: { serverId: string; backup: Backup; onClose: () => void; onSaved: () => void }) {
  const { t } = useTranslation();
  const [note, setNote] = useState(backup.note ?? "");
  const save = useMutation({
    mutationFn: () => backupsApi.update(serverId, backup.id, { note: note.trim() }),
    onSuccess: () => {
      onSaved();
      onClose();
    },
  });

  return (
    <Dialog open onClose={onClose} title={t("backups.editNote")}>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate();
        }}
      >
        <DialogBody>
          <Field label={t("backups.note")} hint={t("backups.noteHint")}>
            <TextInput value={note} maxLength={200} autoComplete="off" data-autofocus onChange={(e) => setNote(e.target.value)} />
          </Field>
          {save.isError && <Notice tone="danger">{errorMessage(t, save.error)}</Notice>}
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose} disabled={save.isPending}>
            {t("ui.cancel")}
          </Button>
          <Button type="submit" variant="primary" loading={save.isPending}>
            {t("backups.saveNote")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
