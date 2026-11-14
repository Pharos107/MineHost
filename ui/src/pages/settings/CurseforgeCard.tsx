import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Flame, Save, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "../../api/client";
import {
  CURSEFORGE_KEY,
  settingsApi,
  type CurseforgeKeySource,
  type CurseforgeSettings,
} from "../../api/settings";
import { Button, Card, Field, Notice, Spinner, Stack, TextInput, type NoticeTone } from "../../ui";

const STATUS_META: Record<CurseforgeKeySource, { tone: NoticeTone; key: string }> = {
  builtin: { tone: "info", key: "appSettings.curseforgeBuiltin" },
  user: { tone: "success", key: "appSettings.curseforgeUser" },
  none: { tone: "warning", key: "appSettings.curseforgeNone" },
};

/** Mục "CurseForge" trong Cài đặt chung (spec P3, mục 3.6 và 6.5). */
export function CurseforgeCard() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [apiKey, setApiKey] = useState("");
  const [saved, setSaved] = useState(false);
  const status = useQuery({ queryKey: CURSEFORGE_KEY, queryFn: settingsApi.curseforge });

  const apply = (data: CurseforgeSettings, didSave: boolean) => {
    queryClient.setQueryData(CURSEFORGE_KEY, data);
    setSaved(didSave);
    if (didSave) setApiKey("");
  };
  const save = useMutation({
    mutationFn: () => settingsApi.setCurseforgeKey(apiKey),
    onSuccess: (data) => apply(data, true),
  });
  const clear = useMutation({
    mutationFn: settingsApi.clearCurseforgeKey,
    onSuccess: (data) => apply(data, false),
  });

  const busy = save.isPending || clear.isPending;
  const failure = save.error ?? clear.error;
  const current = status.data;
  const statusLook = current ? STATUS_META[current.source] : undefined;

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (apiKey.trim() !== "" && !busy) save.mutate();
      }}
    >
      <Card
        id="curseforge"
        title={t("appSettings.curseforge")}
        description={t("appSettings.curseforgeDescription")}
        icon={Flame}
        layout="split"
      >
        <Stack gap={4}>
          {status.isPending ? (
            <Spinner label={t("appSettings.loading")} />
          ) : status.isError ? (
            <Notice tone="danger">{errorMessage(t, status.error)}</Notice>
          ) : (
            statusLook && <Notice tone={statusLook.tone}>{t(statusLook.key)}</Notice>
          )}
          <Field label={t("appSettings.curseforgeKey")} hint={t("appSettings.curseforgeKeyHint")}>
            <TextInput
              type="password"
              autoComplete="off"
              value={apiKey}
              onChange={(e) => {
                setApiKey(e.target.value);
                if (saved) setSaved(false);
              }}
            />
          </Field>
          {failure && <Notice tone="danger">{errorMessage(t, failure)}</Notice>}
          {saved && <Notice tone="success">{t("appSettings.curseforgeSaved")}</Notice>}
          <Stack direction="row" gap={2} justify="end" wrap>
            {current?.source === "user" && (
              <Button variant="danger" icon={Trash2} disabled={busy} loading={clear.isPending} onClick={() => clear.mutate()}>
                {t("appSettings.curseforgeRemove")}
              </Button>
            )}
            <Button type="submit" variant="primary" icon={Save} disabled={apiKey.trim() === "" || busy} loading={save.isPending}>
              {t("appSettings.curseforgeSave")}
            </Button>
          </Stack>
        </Stack>
      </Card>
    </form>
  );
}
