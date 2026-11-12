import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Ban, ListPlus } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "../../../api/client";
import { PLAYER_NAME, playersApi, playersKey, type PlayerActionKind, type Players } from "../../../api/players";
import { Button, Card, Field, Notice, Stack, TextInput } from "../../../ui";

/** Thêm theo tên, để whitelist hoặc cấm cả người chưa vào bao giờ (spec P2, mục 6.7). */
export function AddPlayerCard({ serverId, players }: { serverId: string; players: Players | undefined }) {
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
  const description = players && !players.whitelist_enabled ? t("players.addHintWhitelistOff") : t("players.addHint");

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        submit("whitelist_add");
      }}
    >
      <Card title={t("players.add")} description={description}>
        <Stack gap={4}>
          <Field label={t("players.name")} hint={t("players.nameHint")} error={invalid ? t("players.nameInvalid") : undefined}>
            <TextInput value={name} maxLength={16} autoComplete="off" spellCheck={false} onChange={(e) => setName(e.target.value)} />
          </Field>
          {add.isError && <Notice tone="danger">{errorMessage(t, add.error)}</Notice>}
          {done && <Notice tone="success">{done}</Notice>}
          <Stack direction="row" gap={2} justify="end" wrap>
            <Button type="button" icon={Ban} disabled={busy || trimmed === "" || invalid} loading={busy && add.variables === "ban"} onClick={() => submit("ban")}>
              {t("players.ban")}
            </Button>
            <Button type="submit" variant="primary" icon={ListPlus} disabled={busy || trimmed === "" || invalid} loading={busy && add.variables === "whitelist_add"}>
              {t("players.whitelistAdd")}
            </Button>
          </Stack>
        </Stack>
      </Card>
    </form>
  );
}
