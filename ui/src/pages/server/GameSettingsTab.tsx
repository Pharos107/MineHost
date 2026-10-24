import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { RotateCw, Save, Undo2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type Property, type Server } from "../../api/client";
import { SERVERS_KEY } from "../../api/live";
import { Button, Card, DataTable, Field, Notice, Select, Spinner, Stack, Switch, TextInput, type Column } from "../../ui";
import { COMMON_PROPERTIES, getProperty, MANAGED_KEYS, sameProperties, setProperty, type CommonProperty } from "./properties";

/** Tab "Cài đặt game": các khóa phổ biến của `server.properties` và bảng sửa mọi khóa. */
export function GameSettingsTab({ server }: { server: Server }) {
  const { t } = useTranslation();
  const query = useQuery({ queryKey: [...SERVERS_KEY, server.id, "properties"], queryFn: () => api.properties(server.id) });

  if (query.isPending) return <Spinner label={t("ui.loading")} />;
  if (query.isError) {
    return (
      <Notice
        tone="danger"
        action={
          <Button size="sm" icon={RotateCw} onClick={() => void query.refetch()}>
            {t("common.retry")}
          </Button>
        }
      >
        {errorMessage(t, query.error)}
      </Notice>
    );
  }
  // Lưu xong thì dữ liệu mới về: dựng lại form từ file thật thay vì giữ bản đang sửa.
  return <PropertiesForm key={query.dataUpdatedAt} server={server} initial={query.data.properties} />;
}

function PropertiesForm({ server, initial }: { server: Server; initial: Property[] }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState(initial);
  const save = useMutation({
    mutationFn: () => api.saveProperties(server.id, draft),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: [...SERVERS_KEY, server.id, "properties"] }),
  });
  const set = (key: string, value: string) => setDraft((current) => setProperty(current, key, value));
  const dirty = !sameProperties(draft, initial);

  const columns: Column<Property>[] = [
    { id: "key", header: t("game.key"), cell: (p) => <code>{p.key}</code>, sortValue: (p) => p.key, width: "lg" },
    { id: "value", header: t("game.value"), cell: (p) => <TextInput aria-label={p.key} value={p.value} onChange={(e) => set(p.key, e.target.value)} /> },
  ];

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (dirty) save.mutate();
      }}
    >
      <Stack gap={5}>
        {server.state !== "stopped" && server.state !== "crashed" && <Notice tone="info">{t("game.restartNote")}</Notice>}
        <Card title={t("game.common")}>
          {COMMON_PROPERTIES.map((def) => (
            <CommonField key={def.key} def={def} value={getProperty(draft, def.key) ?? def.fallback} onChange={(v) => set(def.key, v)} />
          ))}
        </Card>
        <Card title={t("game.advanced")} description={t("game.advancedHint")}>
          <DataTable
            label={t("game.advanced")}
            columns={columns}
            rows={draft.filter((p) => !MANAGED_KEYS.has(p.key))}
            rowKey={(p) => p.key}
            rowLabel={(p) => p.key}
            searchText={(p) => p.key}
            empty={<Notice>{t("game.empty")}</Notice>}
          />
        </Card>
        {save.isError && <Notice tone="danger">{errorMessage(t, save.error)}</Notice>}
        {save.isSuccess && !dirty && <Notice tone="success">{t("settings.saved")}</Notice>}
        <Stack direction="row" gap={2} justify="end">
          <Button variant="ghost" icon={Undo2} disabled={!dirty || save.isPending} onClick={() => setDraft(initial)}>
            {t("settings.undo")}
          </Button>
          <Button type="submit" variant="primary" icon={Save} disabled={!dirty} loading={save.isPending}>
            {t("settings.save")}
          </Button>
        </Stack>
      </Stack>
    </form>
  );
}

function CommonField({ def, value, onChange }: { def: CommonProperty; value: string; onChange: (value: string) => void }) {
  const { t } = useTranslation();
  const label = t(`game.fields.${def.key}`);
  const hint = t(`game.hints.${def.key}`, { defaultValue: "" }) || undefined;

  if (def.kind === "boolean") {
    return <Switch label={label} description={hint} checked={value === "true"} onChange={(on) => onChange(String(on))} />;
  }
  return (
    <Field label={label} hint={hint}>
      {def.kind === "select" ? (
        <Select value={value} onChange={(e) => onChange(e.target.value)}>
          {def.options?.map((option) => (
            <option key={option} value={option}>
              {t(`game.options.${option}`)}
            </option>
          ))}
        </Select>
      ) : (
        <TextInput type={def.kind === "number" ? "number" : "text"} value={value} onChange={(e) => onChange(e.target.value)} />
      )}
    </Field>
  );
}
