import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type Server } from "../../../api/client";
import { SERVERS_KEY } from "../../../api/live";
import { Button, Card, Dialog, DialogBody, DialogFooter, Field, InfoTip, Notice, Select, Stack, TextInput } from "../../../ui";
import {
  DEFAULT_CHART_MINUTES,
  MAX_OVERVIEW_MINUTES,
  MAX_RETENTION_MINUTES,
  MIN_CHART_MINUTES,
  fromDurationParts,
  toDurationParts,
  type ChartKind,
  type DurationUnit,
} from "./chartPrefs";
import { metricsKey } from "./metricsLive";

interface ChartSettingsDialogProps {
  open: boolean;
  onClose: () => void;
  kind: ChartKind;
  title: string;
  server: Server;
  windowMinutes: number;
  retentionMinutes: number;
  onSaveWindow: (kind: ChartKind, minutes: number) => void;
}

const PRESETS = [
  { minutes: 30, key: "overview.preset30m" },
  { minutes: 60, key: "overview.preset1h" },
  { minutes: 720, key: "overview.preset12h" },
  { minutes: 1440, key: "overview.preset1d" },
] as const;

function isPreset(minutes: number): boolean {
  return PRESETS.some((p) => p.minutes === minutes);
}

export function ChartSettingsDialog({
  open,
  onClose,
  kind,
  title,
  server,
  windowMinutes,
  retentionMinutes,
  onSaveWindow,
}: ChartSettingsDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog open={open} onClose={onClose} title={t("overview.chartSettingsTitle", { title })}>
      {open && (
        <SettingsForm
          kind={kind}
          server={server}
          initialWindow={windowMinutes}
          initialRetention={retentionMinutes}
          onSaveWindow={onSaveWindow}
          onClose={onClose}
        />
      )}
    </Dialog>
  );
}

function SettingsForm({
  kind,
  server,
  initialWindow,
  initialRetention,
  onSaveWindow,
  onClose,
}: {
  kind: ChartKind;
  server: Server;
  initialWindow: number;
  initialRetention: number;
  onSaveWindow: (kind: ChartKind, minutes: number) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [winParts, setWinParts] = useState(() => toDurationParts(initialWindow));
  const [winRaw, setWinRaw] = useState(() => String(toDurationParts(initialWindow).value));
  const [winCustom, setWinCustom] = useState(() => !isPreset(initialWindow));
  const [retParts, setRetParts] = useState(() => toDurationParts(initialRetention));
  const [retRaw, setRetRaw] = useState(() => String(toDurationParts(initialRetention).value));
  const [retCustom, setRetCustom] = useState(() => !isPreset(initialRetention));

  const parsedWin = parseMinutes(winRaw, winParts.unit, MIN_CHART_MINUTES, MAX_OVERVIEW_MINUTES);
  const parsedRet = parseMinutes(retRaw, retParts.unit, MIN_CHART_MINUTES, MAX_RETENTION_MINUTES);
  const valid = parsedWin !== null && parsedRet !== null;

  const save = useMutation({
    mutationFn: (minutes: number) => api.updateServer(server.id, { metrics_retention_minutes: minutes }),
    onSuccess: (updated) => {
      queryClient.setQueryData([...SERVERS_KEY, server.id], updated);
      void queryClient.invalidateQueries({ queryKey: metricsKey(server.id) });
      onClose();
    },
  });

  function applyWindowPreset(minutes: number) {
    const parts = toDurationParts(minutes);
    setWinParts(parts);
    setWinRaw(String(parts.value));
    setWinCustom(false);
  }

  function applyRetentionPreset(minutes: number) {
    const parts = toDurationParts(minutes);
    setRetParts(parts);
    setRetRaw(String(parts.value));
    setRetCustom(false);
  }

  function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (parsedWin === null || parsedRet === null) return;
    onSaveWindow(kind, parsedWin);
    if (parsedRet !== initialRetention) {
      save.mutate(parsedRet);
      return;
    }
    onClose();
  }

  return (
    <form onSubmit={onSubmit}>
      <DialogBody>
        <Card
          variant="subtle"
          title={t("overview.displayWindow")}
          actions={<InfoTip label={t("overview.displayWindow")} text={t("overview.displayWindowHelp")} />}
        >
          <DurationControl
            customOpen={winCustom}
            raw={winRaw}
            unit={winParts.unit}
            error={parsedWin === null ? t("overview.windowInvalid") : undefined}
            activeMinutes={parsedWin ?? DEFAULT_CHART_MINUTES}
            onOpenCustom={() => setWinCustom(true)}
            onRawChange={setWinRaw}
            onUnitChange={(unit) => setWinParts((c) => ({ ...c, unit }))}
            onPreset={applyWindowPreset}
          />
        </Card>

        <Card
          variant="subtle"
          title={t("overview.retentionTitle")}
          actions={<InfoTip label={t("overview.retentionTitle")} text={t("overview.retentionHelp")} />}
        >
          <DurationControl
            customOpen={retCustom}
            raw={retRaw}
            unit={retParts.unit}
            error={parsedRet === null ? t("overview.retentionInvalid") : undefined}
            activeMinutes={parsedRet ?? initialRetention}
            onOpenCustom={() => setRetCustom(true)}
            onRawChange={setRetRaw}
            onUnitChange={(unit) => setRetParts((c) => ({ ...c, unit }))}
            onPreset={applyRetentionPreset}
          />
        </Card>

        {parsedWin !== null && parsedRet !== null && parsedWin > parsedRet && (
          <Notice tone="info">{t("overview.windowExceedsRetention")}</Notice>
        )}
        {save.isError && <Notice tone="danger">{errorMessage(t, save.error)}</Notice>}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" onClick={onClose} disabled={save.isPending}>
          {t("ui.cancel")}
        </Button>
        <Button type="submit" variant="primary" disabled={!valid} loading={save.isPending}>
          {t("settings.save")}
        </Button>
      </DialogFooter>
    </form>
  );
}

function DurationControl({
  customOpen,
  raw,
  unit,
  error,
  activeMinutes,
  onOpenCustom,
  onRawChange,
  onUnitChange,
  onPreset,
}: {
  customOpen: boolean;
  raw: string;
  unit: DurationUnit;
  error?: string;
  activeMinutes: number;
  onOpenCustom: () => void;
  onRawChange: (value: string) => void;
  onUnitChange: (unit: DurationUnit) => void;
  onPreset: (minutes: number) => void;
}) {
  const { t } = useTranslation();
  const unitId = useId();
  return (
    <Stack gap={3}>
      <Stack direction="row" gap={2} wrap>
        {PRESETS.map((p) => (
          <Button
            key={p.minutes}
            size="sm"
            variant={!customOpen && activeMinutes === p.minutes ? "primary" : "secondary"}
            onClick={() => onPreset(p.minutes)}
          >
            {t(p.key)}
          </Button>
        ))}
        <Button size="sm" variant={customOpen ? "primary" : "secondary"} onClick={onOpenCustom}>
          {t("overview.custom")}
        </Button>
      </Stack>
      {customOpen && (
        <Field label={t("overview.customDuration")} error={error}>
          <Stack direction="row" gap={2} align="center">
            <TextInput
              type="number"
              inputMode="numeric"
              min={1}
              step={1}
              value={raw}
              onChange={(e) => onRawChange(e.target.value)}
            />
            <Select id={unitId} aria-label={t("overview.customDuration")} value={unit} onChange={(e) => onUnitChange(e.target.value as DurationUnit)}>
              <option value="minutes">{t("overview.unitMinutes")}</option>
              <option value="hours">{t("overview.unitHours")}</option>
              <option value="days">{t("overview.unitDays")}</option>
            </Select>
          </Stack>
        </Field>
      )}
    </Stack>
  );
}

function parseMinutes(raw: string, unit: DurationUnit, min: number, max: number): number | null {
  const trimmed = raw.trim();
  if (!/^\d+$/.test(trimmed)) return null;
  const n = Number(trimmed);
  if (!Number.isSafeInteger(n) || n < 1) return null;
  const total = fromDurationParts(n, unit);
  if (!Number.isFinite(total) || total < min || total > max) return null;
  return total;
}
