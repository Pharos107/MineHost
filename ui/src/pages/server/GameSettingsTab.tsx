import { useMutation, useQuery, useQueryClient, type UseMutationResult } from "@tanstack/react-query";
import { RotateCw, Save, Undo2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type Property, type Server } from "../../api/client";
import { filesApi } from "../../api/files";
import { SERVERS_KEY } from "../../api/live";
import {
  ActionBar,
  Button,
  Card,
  Columns,
  Field,
  MotdPreview,
  Notice,
  Select,
  Spinner,
  Stack,
  Switch,
  TextInput,
} from "../../ui";
import { AddPropertyDialog, AdvancedPropertiesCard } from "./AdvancedPropertiesCard";
import {
  COMMON_PROPERTIES,
  countChangedProperties,
  getProperty,
  removeProperty,
  sameProperties,
  setProperty,
  type CommonProperty,
} from "./properties";
import { isModServer, type AdvancedFilter } from "./propertiesMeta";

/** Tab "Cài đặt game": các khóa phổ biến của `server.properties` và bảng sửa mọi khóa. */
export function GameSettingsTab({ server }: { server: Server }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const key = [...SERVERS_KEY, server.id, "properties"];
  // Không tự refetch khi chuyển cửa sổ trình duyệt để tránh dựng lại form làm mất phần đang sửa dở.
  const query = useQuery({
    queryKey: key,
    queryFn: () => api.properties(server.id),
    refetchOnWindowFocus: false,
  });
  const rootFiles = useQuery({
    queryKey: ["files", server.id, ""],
    queryFn: () => filesApi.list(server.id, ""),
  });
  const iconEntry = Array.isArray(rootFiles.data)
    ? rootFiles.data.find((entry) => entry.name === "server-icon.png" && entry.kind === "file")
    : undefined;
  const iconUrl = iconEntry
    ? `${filesApi.downloadUrl(server.id, "server-icon.png")}&v=${encodeURIComponent(iconEntry.modified ?? String(iconEntry.size))}`
    : undefined;

  // Mutation nằm ở đây để thông báo "Đã lưu." còn lại sau khi form được dựng lại.
  const save = useMutation({
    mutationFn: (draft: Property[]) => api.saveProperties(server.id, draft),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: key }),
  });

  const saveAndRestart = useMutation({
    mutationFn: async (draft: Property[]) => {
      await api.saveProperties(server.id, draft);
      // Làm mới bản đã lưu ngay trước khi restart để nếu restart lỗi thì form không còn báo chưa lưu.
      await queryClient.invalidateQueries({ queryKey: key });
      await api.action(server.id, "restart");
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: [...SERVERS_KEY, server.id] });
    },
  });

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
  return (
    <PropertiesForm
      key={query.dataUpdatedAt}
      server={server}
      iconUrl={iconUrl}
      initial={query.data.properties}
      save={save}
      saveAndRestart={saveAndRestart}
    />
  );
}

interface PropertiesFormProps {
  server: Server;
  iconUrl?: string;
  initial: Property[];
  save: UseMutationResult<void, Error, Property[]>;
  saveAndRestart: UseMutationResult<void, Error, Property[]>;
}

function PropertiesForm({ server, iconUrl, initial, save, saveAndRestart }: PropertiesFormProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState(initial);
  const [filter, setFilter] = useState<AdvancedFilter>("all");
  const [adding, setAdding] = useState(false);
  const set = (key: string, value: string) => setDraft((current) => setProperty(current, key, value));
  const remove = (key: string) => setDraft((current) => removeProperty(current, key));

  const dirty = !sameProperties(draft, initial);
  useLeaveWarning(dirty);
  const changedCount = dirty ? countChangedProperties(draft, initial) : 0;
  const busy = save.isPending || saveAndRestart.isPending;
  const error = saveAndRestart.isError ? saveAndRestart.error : save.isError ? save.error : null;
  const saved = !dirty && (save.isSuccess || saveAndRestart.isSuccess);

  const motdDef = COMMON_PROPERTIES.find((p) => p.key === "motd")!;
  const gridFields = COMMON_PROPERTIES.filter((p) => p.key !== "motd" && p.kind !== "boolean");
  const switchFields = COMMON_PROPERTIES.filter((p) => p.kind === "boolean");

  const allowFlight = getProperty(draft, "allow-flight") ?? "false";
  const onlineMode = getProperty(draft, "online-mode") ?? "true";
  const whiteList = getProperty(draft, "white-list") ?? "false";

  const statusText = dirty
    ? t("game.unsavedCount", { count: changedCount })
    : saved
      ? t("settings.saved")
      : undefined;

  const existingKeys = new Set(draft.map((p) => p.key));

  return (
    <>
      <form
        noValidate
        onSubmit={(e) => {
          e.preventDefault();
          if (dirty && !busy) save.mutate(draft);
        }}
      >
        <Stack gap={5}>
          {server.state !== "stopped" && server.state !== "crashed" && <Notice tone="info">{t("game.restartNote")}</Notice>}
          {isModServer(server) && allowFlight === "false" && (
            <Notice
              tone="warning"
              action={
                <Button size="sm" onClick={() => set("allow-flight", "true")}>
                  {t("game.enableNow")}
                </Button>
              }
            >
              {t("game.modFlightWarning")}
            </Notice>
          )}
          {onlineMode === "false" && whiteList === "false" && (
            <Notice
              tone="warning"
              action={
                <Button size="sm" onClick={() => set("white-list", "true")}>
                  {t("game.enableWhitelistNow")}
                </Button>
              }
            >
              {t("game.offlineWhitelistWarning")}
            </Notice>
          )}
          <Card title={t("game.common")}>
            <Field label={t("game.fields.motd")} hint={t("game.hints.motd")}>
              <Stack gap={2}>
                <TextInput
                  value={getProperty(draft, "motd") ?? motdDef.fallback}
                  onChange={(e) => set("motd", e.target.value)}
                />
                <MotdPreview
                  serverId={server.id}
                  serverName={server.name || undefined}
                  maxPlayers={getProperty(draft, "max-players") ?? "20"}
                  iconUrl={iconUrl}
                  value={getProperty(draft, "motd") ?? ""}
                  fallback={motdDef.fallback}
                  label={t("game.motdPreview")}
                />
              </Stack>
            </Field>
            <Columns count={2}>
              {gridFields.map((def) => (
                <CommonField
                  key={def.key}
                  def={def}
                  value={getProperty(draft, def.key) ?? def.fallback}
                  onChange={(v) => set(def.key, v)}
                />
              ))}
            </Columns>
            {switchFields.map((def) => (
              <CommonField
                key={def.key}
                def={def}
                value={getProperty(draft, def.key) ?? def.fallback}
                onChange={(v) => set(def.key, v)}
              />
            ))}
          </Card>
          <AdvancedPropertiesCard
            draft={draft}
            initial={initial}
            filter={filter}
            onFilterChange={setFilter}
            onSet={set}
            onRemove={remove}
            onAddClick={() => setAdding(true)}
          />
          {error && <Notice tone="danger">{errorMessage(t, error)}</Notice>}
          <ActionBar dirty={dirty} status={statusText}>
            <Button variant="ghost" icon={Undo2} disabled={!dirty || busy} onClick={() => setDraft(initial)}>
              {t("settings.undo")}
            </Button>
            {server.state === "running" && (
              <Button
                icon={RotateCw}
                disabled={!dirty || busy}
                loading={saveAndRestart.isPending}
                onClick={() => saveAndRestart.mutate(draft)}
              >
                {t("game.saveAndRestart")}
              </Button>
            )}
            <Button type="submit" variant="primary" icon={Save} disabled={!dirty || busy} loading={save.isPending}>
              {t("settings.save")}
            </Button>
          </ActionBar>
        </Stack>
      </form>
      <AddPropertyDialog
        open={adding}
        existingKeys={existingKeys}
        onClose={() => setAdding(false)}
        onAdd={(key, value) => {
          set(key, value);
          setFilter("all");
          setAdding(false);
        }}
      />
    </>
  );
}

/** Hỏi lại khi đóng tab hoặc tải lại trang mà còn thay đổi chưa lưu. */
function useLeaveWarning(dirty: boolean) {
  useEffect(() => {
    if (!dirty) return;
    const onBeforeUnload = (e: BeforeUnloadEvent) => e.preventDefault();
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, [dirty]);
}

function CommonField({ def, value, onChange }: { def: CommonProperty; value: string; onChange: (value: string) => void }) {
  const { t } = useTranslation();
  const label = t(`game.fields.${def.key}`);
  const hint = t(`game.hints.${def.key}`, { defaultValue: "" }) || undefined;

  if (def.kind === "boolean") {
    return <Switch label={label} description={hint} checked={value === "true"} onChange={(on) => onChange(String(on))} />;
  }
  const selectOptions = def.kind === "select" ? (def.options?.includes(value) ? def.options : [value, ...(def.options ?? [])]) : undefined;
  return (
    <Field label={label} hint={hint}>
      {def.kind === "select" && selectOptions ? (
        <Select value={value} onChange={(e) => onChange(e.target.value)}>
          {selectOptions.map((option) => (
            <option key={option} value={option}>
              {t(`game.options.${option}`, { defaultValue: option || t("game.emptyOption") })}
            </option>
          ))}
        </Select>
      ) : (
        <TextInput
          type={def.kind === "number" ? "number" : "text"}
          inputMode={def.kind === "number" ? "numeric" : undefined}
          min={def.min}
          max={def.max}
          value={value}
          onChange={(e) => onChange(e.target.value)}
        />
      )}
    </Field>
  );
}
