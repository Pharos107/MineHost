import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus, Save, Undo2, X } from "lucide-react";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type Server } from "../../api/client";
import { SERVERS_KEY } from "../../api/live";
import { formatMemoryMb } from "../../format";
import { MEMORY_STEP_MB, memoryRange } from "../../memory";
import { Button, Card, Field, IconButton, Notice, Slider, Stack, Switch, TextInput } from "../../ui";
import { DeleteServer } from "./DeleteServer";
import { MAX_RESTART_TIMES, settingsErrors, settingsFrom, settingsPatch, type SettingsForm } from "./settings";

/** Tab "Cài đặt server": tên, RAM, cổng, tự bật, tự khởi động lại khi crash, xóa server. */
export function ServerSettingsTab({ server }: { server: Server }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [form, setForm] = useState(() => settingsFrom(server));
  const save = useMutation({
    mutationFn: () => api.updateServer(server.id, settingsPatch(server, form)),
    onSuccess: (updated) => {
      queryClient.setQueryData([...SERVERS_KEY, server.id], updated);
      setForm(settingsFrom(updated));
    },
  });
  const update = (patch: Partial<SettingsForm>) => setForm((current) => ({ ...current, ...patch }));
  const errors = settingsErrors(form);
  const dirty = Object.keys(settingsPatch(server, form)).length > 0;
  const valid = !errors.name && !errors.port && !errors.restartAt;

  return (
    <Stack gap={5}>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (dirty && valid) save.mutate();
        }}
      >
        <Card title={t("settings.title")}>
          {server.state !== "stopped" && server.state !== "crashed" && <Notice tone="info">{t("game.restartNote")}</Notice>}
          <Field label={t("wizard.name")} error={errors.name && t(errors.name)}>
            <TextInput value={form.name} onChange={(e) => update({ name: e.target.value })} autoComplete="off" />
          </Field>
          <MemoryField value={form.memoryMb} onChange={(memoryMb) => update({ memoryMb })} />
          <Field label={t("wizard.port")} error={errors.port && t(errors.port)}>
            <TextInput type="number" inputMode="numeric" value={form.port} onChange={(e) => update({ port: e.target.value })} />
          </Field>
          <Switch label={t("settings.autostart")} description={t("settings.autostartHint")} checked={form.autostart} onChange={(autostart) => update({ autostart })} />
          <Switch
            label={t("settings.restartOnCrash")}
            description={t("settings.restartOnCrashHint")}
            checked={form.restartOnCrash}
            onChange={(restartOnCrash) => update({ restartOnCrash })}
          />
          <RestartTimesField times={form.restartAt} error={errors.restartAt && t(errors.restartAt)} onChange={(restartAt) => update({ restartAt })} />
          {save.isError && <Notice tone="danger">{errorMessage(t, save.error)}</Notice>}
          {save.isSuccess && !dirty && <Notice tone="success">{t("settings.saved")}</Notice>}
          <Stack direction="row" gap={2} justify="end">
            <Button variant="ghost" icon={Undo2} disabled={!dirty || save.isPending} onClick={() => setForm(settingsFrom(server))}>
              {t("settings.undo")}
            </Button>
            <Button type="submit" variant="primary" icon={Save} disabled={!dirty || !valid} loading={save.isPending}>
              {t("settings.save")}
            </Button>
          </Stack>
        </Card>
      </form>
      <DeleteServer server={server} />
    </Stack>
  );
}

/** Giờ khởi động lại hằng ngày (spec P2, mục 6.9): mỗi mốc một ô chọn giờ, tối đa 4 mốc. */
function RestartTimesField({ times, error, onChange }: { times: string[]; error?: string; onChange: (times: string[]) => void }) {
  const { t } = useTranslation();
  const baseId = useId();
  const set = (index: number, value: string) => onChange(times.map((time, i) => (i === index ? value : time)));
  return (
    <Field label={t("settings.restartAt")} hint={t("settings.restartAtHint")} error={error}>
      <Stack gap={2}>
        {times.length === 0 && <span>{t("settings.restartAtNone")}</span>}
        {times.map((time, index) => (
          <Stack key={index} direction="row" gap={2} align="center">
            {/* Ô đầu nhận `id` của `Field` để nhãn trỏ tới nó; các ô sau cần `id` riêng. */}
            <TextInput
              type="time"
              {...(index === 0 ? {} : { id: `${baseId}-${index}` })}
              aria-label={t("settings.restartAtTime", { n: index + 1 })}
              value={time}
              onChange={(e) => set(index, e.target.value)}
            />
            <IconButton icon={X} label={t("settings.restartAtRemove", { time })} onClick={() => onChange(times.filter((_, i) => i !== index))} />
          </Stack>
        ))}
        <Stack direction="row">
          <Button size="sm" icon={Plus} disabled={times.length >= MAX_RESTART_TIMES} onClick={() => onChange([...times, "04:00"])}>
            {t("settings.restartAtAdd")}
          </Button>
        </Stack>
      </Stack>
    </Field>
  );
}

function MemoryField({ value, onChange }: { value: number; onChange: (mb: number) => void }) {
  const { t, i18n } = useTranslation();
  const system = useQuery({ queryKey: ["system"], queryFn: api.system });
  if (!system.data) return null;
  const suggested = system.data.suggested_memory_mb.max;
  const range = memoryRange(system.data.memory_total_bytes, system.data.suggested_memory_mb);
  return (
    <Field label={t("wizard.memory")} hint={t("wizard.memoryHint")}>
      <Slider
        value={value}
        onChange={onChange}
        min={Math.min(range.min, value)}
        max={Math.max(range.max, value)}
        step={MEMORY_STEP_MB}
        recommended={suggested}
        formatValue={(mb) => formatMemoryMb(mb, i18n.language)}
      />
    </Field>
  );
}
