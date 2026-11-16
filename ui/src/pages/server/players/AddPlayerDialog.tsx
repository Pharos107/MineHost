import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Ban, ListPlus } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "../../../api/client";
import { PLAYER_NAME, playersApi, playersKey, type PlayerActionKind } from "../../../api/players";
import { Button, Dialog, DialogBody, DialogFooter, Field, Notice, TextInput } from "../../../ui";

interface AddPlayerDialogProps {
  open: boolean;
  onClose: () => void;
  serverId: string;
  /** `false` thì nhắc rằng ai cũng vào được, whitelist chỉ có tác dụng khi bật. */
  whitelistEnabled: boolean;
}

/**
 * Thêm theo tên, để whitelist hoặc cấm cả người chưa vào bao giờ (spec P2, mục 6.7). Hộp thoại
 * giữ mở sau mỗi lần thêm để thêm liền nhiều người.
 */
export function AddPlayerDialog({ open, onClose, serverId, whitelistEnabled }: AddPlayerDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog open={open} onClose={onClose} title={t("players.add")}>
      <AddPlayerForm serverId={serverId} whitelistEnabled={whitelistEnabled} />
    </Dialog>
  );
}

function AddPlayerForm({ serverId, whitelistEnabled }: { serverId: string; whitelistEnabled: boolean }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [done, setDone] = useState<string | null>(null);
  const trimmed = name.trim();
  const invalid = trimmed !== "" && !PLAYER_NAME.test(trimmed);
  const add = useMutation({
    mutationFn: (action: PlayerActionKind) => playersApi.act(serverId, trimmed, { action }),
    onSuccess: (result, action) => {
      setDone(t(`players.added.${action === "ban" ? "ban" : "whitelist"}.${result.done === "sent" ? "sent" : "saved"}`, { name: trimmed }));
      setName("");
      void queryClient.invalidateQueries({ queryKey: playersKey(serverId) });
    },
  });
  const submit = (action: PlayerActionKind) => {
    setDone(null);
    if (PLAYER_NAME.test(trimmed)) add.mutate(action);
  };
  const busy = add.isPending;
  const blocked = busy || trimmed === "" || invalid;

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        submit("whitelist_add");
      }}
    >
      <DialogBody>
        <p>{whitelistEnabled ? t("players.addHint") : t("players.addHintWhitelistOff")}</p>
        <Field label={t("players.name")} hint={t("players.nameHint")} error={invalid ? t("players.nameInvalid") : undefined}>
          <TextInput value={name} maxLength={16} autoComplete="off" spellCheck={false} data-autofocus onChange={(e) => setName(e.target.value)} />
        </Field>
        {add.isError && <Notice tone="danger">{errorMessage(t, add.error)}</Notice>}
        {done && <Notice tone="success">{done}</Notice>}
      </DialogBody>
      <DialogFooter>
        <Button type="button" icon={Ban} disabled={blocked} loading={busy && add.variables === "ban"} onClick={() => submit("ban")}>
          {t("players.ban")}
        </Button>
        <Button type="submit" variant="primary" icon={ListPlus} disabled={blocked} loading={busy && add.variables === "whitelist_add"}>
          {t("players.whitelistAdd")}
        </Button>
      </DialogFooter>
    </form>
  );
}
