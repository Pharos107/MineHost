import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Archive, Save } from "lucide-react";
import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

import { backupsApi, backupsKey } from "../../../api/backups";
import { api, errorMessage, type Server } from "../../../api/client";
import { SERVERS_KEY } from "../../../api/live";
import { Button, Card, Field, Notice, Select, Stack, Switch, TextInput } from "../../../ui";
import { JobLine } from "./JobLine";
import { scheduleErrors, scheduleForm, scheduleString, type ScheduleForm } from "./schedule";

/** Thẻ "Sao lưu ngay": ghi chú, bảo vệ, nút Sao lưu, rồi tiến độ Job ngay trong thẻ. */
export function BackupNowCard({ server }: { server: Server }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [note, setNote] = useState("");
  const [isProtected, setProtected] = useState(false);
  const [jobId, setJobId] = useState<number | null>(null);
  const start = useMutation({
    mutationFn: () => backupsApi.create(server.id, note, isProtected),
    onSuccess: (started) => setJobId(started.job_id),
  });
  const done = useCallback(() => {
    setNote("");
    setProtected(false);
    void queryClient.invalidateQueries({ queryKey: backupsKey(server.id) });
  }, [queryClient, server.id]);

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        start.mutate();
      }}
    >
      <Card title={t("backups.now")} description={t("backups.nowHint")}>
        <Stack gap={4}>
          <Field label={t("backups.note")} hint={t("backups.noteHint")}>
            <TextInput value={note} maxLength={200} autoComplete="off" onChange={(e) => setNote(e.target.value)} />
          </Field>
          <Switch label={t("backups.protect")} description={t("backups.protectHint")} checked={isProtected} onChange={setProtected} />
          {start.isError && <Notice tone="danger">{errorMessage(t, start.error)}</Notice>}
          {jobId !== null && <JobLine key={jobId} jobId={jobId} onDone={done} />}
          <Stack direction="row" justify="end">
            <Button type="submit" variant="primary" icon={Archive} loading={start.isPending}>
              {t("backups.create")}
            </Button>
          </Stack>
        </Stack>
      </Card>
    </form>
  );
}

/** Thẻ "Tự động": lịch và số bản giữ lại, lưu vào `PATCH /servers/{id}`. */
export function ScheduleCard({ server }: { server: Server }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const initial = scheduleForm(server.backup?.schedule ?? "off", server.backup?.keep ?? 10);
  const [form, setForm] = useState<ScheduleForm>(initial);
  const errors = scheduleErrors(form);
  const valid = !errors.hours && !errors.time && !errors.keep;
  const save = useMutation({
    mutationFn: () => api.updateServer(server.id, { backup: { schedule: scheduleString(form), keep: Number(form.keep) } }),
    onSuccess: (updated) => queryClient.setQueryData([...SERVERS_KEY, server.id], updated),
  });
  const update = (patch: Partial<ScheduleForm>) => setForm((current) => ({ ...current, ...patch }));

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (valid) save.mutate();
      }}
    >
      <Card title={t("backups.auto")} description={t("backups.autoHint")}>
        <Stack gap={4}>
          <Field label={t("backups.schedule")}>
            <Select value={form.mode} onChange={(e) => update({ mode: e.target.value as ScheduleForm["mode"] })}>
              <option value="off">{t("backups.modeOff")}</option>
              <option value="every">{t("backups.modeEvery")}</option>
              <option value="daily">{t("backups.modeDaily")}</option>
            </Select>
          </Field>
          {form.mode === "every" && (
            <Field label={t("backups.hours")} error={errors.hours && t(errors.hours)}>
              <TextInput type="number" inputMode="numeric" value={form.hours} onChange={(e) => update({ hours: e.target.value })} />
            </Field>
          )}
          {form.mode === "daily" && (
            <Field label={t("backups.time")} hint={t("backups.timeHint")} error={errors.time && t(errors.time)}>
              <TextInput type="time" value={form.time} onChange={(e) => update({ time: e.target.value })} />
            </Field>
          )}
          <Field label={t("backups.keep")} error={errors.keep && t(errors.keep)}>
            <TextInput type="number" inputMode="numeric" value={form.keep} onChange={(e) => update({ keep: e.target.value })} />
          </Field>
          {save.isError && <Notice tone="danger">{errorMessage(t, save.error)}</Notice>}
          {save.isSuccess && <Notice tone="success">{t("backups.saved")}</Notice>}
          <Stack direction="row" justify="end">
            <Button type="submit" variant="primary" icon={Save} disabled={!valid} loading={save.isPending}>
              {t("backups.save")}
            </Button>
          </Stack>
        </Stack>
      </Card>
    </form>
  );
}
