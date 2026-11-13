import type { UseQueryResult } from "@tanstack/react-query";
import { useQuery } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, RotateCw } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type CatalogVersion, type Loader } from "../../api/client";
import { formatBytes, formatMemoryMb } from "../../format";
import { exceedsAvailable, MEMORY_STEP_MB, memoryRange } from "../../memory";
import { Badge, Button, ChoiceGroup, Field, Notice, Select, Slider, Spinner, Stack, Switch, TextInput } from "../../ui";
import { isModLoader, MOD_LOADERS, nameError, portError, STANDARD_LOADERS, type WizardForm } from "./form";

/** Số bản mới nhất hiện sẵn; các bản cũ hơn nằm sau công tắc "Hiện bản cũ hơn". */
const RECENT_VERSIONS = 5;

function LoadError({ error, retry }: { error: unknown; retry: () => void }) {
  const { t } = useTranslation();
  return (
    <Notice
      tone="danger"
      action={
        <Button size="sm" icon={RotateCw} onClick={retry}>
          {t("common.retry")}
        </Button>
      }
    >
      {errorMessage(t, error)}
    </Notice>
  );
}

const ALL_LOADERS: readonly Loader[] = [...STANDARD_LOADERS, ...MOD_LOADERS];

function isLoader(value: string): value is Loader {
  return (ALL_LOADERS as readonly string[]).includes(value);
}

/** Bước 1: chọn kiểu server (thường, có mod, modpack, nhập có sẵn) và loader (spec P3, mục 6.1). */
export function LoaderStep({ value, onChange }: { value: Loader | "import"; onChange: (value: Loader | "import") => void }) {
  const { t } = useTranslation();
  const kind = value === "import" ? "import" : isModLoader(value) ? "modded" : "standard";
  const loaders = kind === "modded" ? MOD_LOADERS : STANDARD_LOADERS;

  return (
    <Stack gap={5}>
      <ChoiceGroup
        label={t("wizard.kind")}
        value={kind}
        onChange={(next) => {
          if (next === "standard") onChange("paper");
          else if (next === "modded") onChange("fabric");
          else if (next === "import") onChange("import");
        }}
        options={[
          { value: "standard", label: t("wizard.kindStandard"), description: t("wizard.kindStandardDescription") },
          { value: "modded", label: t("wizard.kindModded"), description: t("wizard.kindModdedDescription") },
          {
            value: "modpack",
            label: t("wizard.kindModpack"),
            description: t("wizard.kindModpackDescription"),
            badge: <Badge>{t("wizard.soon")}</Badge>,
            disabled: true,
          },
          { value: "import", label: t("wizard.importLabel"), description: t("wizard.importDescription") },
        ]}
      />
      {kind !== "import" && (
        <ChoiceGroup
          label={t("wizard.loader")}
          value={value}
          onChange={(next) => {
            if (isLoader(next)) onChange(next);
          }}
          options={loaders.map((loader) => ({
            value: loader,
            label: t(`loaders.${loader}`),
            description: t(`wizard.${loader}Description`),
            badge: loader === "paper" || loader === "fabric" ? <Badge tone="primary">{t("wizard.recommended")}</Badge> : undefined,
          }))}
        />
      )}
    </Stack>
  );
}

interface VersionStepProps {
  versions: UseQueryResult<CatalogVersion[]>;
  value: string;
  onChange: (version: string) => void;
  loader?: Loader;
  loaderVersion?: string;
  onLoaderVersionChange?: (version: string) => void;
}

export function VersionStep({ versions, value, onChange, loader, loaderVersion, onLoaderVersionChange }: VersionStepProps) {
  const { t } = useTranslation();
  const list = versions.data ?? [];
  const [showOlder, setShowOlder] = useState(() => list.findIndex((v) => v.id === value) >= RECENT_VERSIONS);
  const [advanced, setAdvanced] = useState(() => Boolean(loaderVersion));
  const modded = Boolean(loader && isModLoader(loader));
  const loaderVersions = useQuery({
    queryKey: ["catalog", loader, value, "loader-versions"],
    queryFn: () => api.loaderVersions(loader!, value),
    enabled: modded && value !== "",
    staleTime: 5 * 60_000,
  });

  if (versions.isPending) return <Spinner label={t("ui.loading")} />;
  if (versions.isError) return <LoadError error={versions.error} retry={() => void versions.refetch()} />;
  if (list.length === 0) return <Notice tone="warning">{t("wizard.noVersions")}</Notice>;

  const shown = showOlder ? list : list.slice(0, RECENT_VERSIONS);
  const toggle = (on: boolean) => {
    setShowOlder(on);
    // Ẩn bản cũ đang chọn thì quay về bản mới nhất để ô chọn không trống.
    if (!on && !list.slice(0, RECENT_VERSIONS).some((v) => v.id === value)) onChange(list[0]?.id ?? "");
  };
  const loaderList = loaderVersions.data ?? [];
  const chosenLoader = loaderVersion || loaderList[0]?.version || "";

  return (
    <>
      <Field label={t("wizard.version")} hint={t("wizard.versionHint")}>
        <Select value={value} onChange={(e) => onChange(e.target.value)}>
          {shown.map((version, index) => (
            <option key={version.id} value={version.id}>
              {index === 0 ? t("wizard.latest", { version: version.id }) : version.id}
            </option>
          ))}
        </Select>
      </Field>
      {list.length > RECENT_VERSIONS && <Switch label={t("wizard.showOlder")} checked={showOlder} onChange={toggle} />}
      {modded && (
        <>
          <div>
            <Button variant="ghost" size="sm" icon={advanced ? ChevronDown : ChevronRight} aria-expanded={advanced} onClick={() => setAdvanced(!advanced)}>
              {t("wizard.advanced")}
            </Button>
          </div>
          {advanced && (
            <>
              {loaderVersions.isPending && <Spinner label={t("ui.loading")} />}
              {loaderVersions.isError && <LoadError error={loaderVersions.error} retry={() => void loaderVersions.refetch()} />}
              {loaderVersions.isSuccess && loaderList.length > 0 && (
                <Field label={t("wizard.loaderVersion")} hint={t("wizard.loaderVersionHint")}>
                  <Select value={chosenLoader} onChange={(e) => onLoaderVersionChange?.(e.target.value)}>
                    {loaderList.map((item, index) => (
                      <option key={item.version} value={item.version}>
                        {index === 0 ? t("wizard.latest", { version: item.version }) : item.version}
                      </option>
                    ))}
                  </Select>
                </Field>
              )}
            </>
          )}
        </>
      )}
    </>
  );
}

interface SettingsStepProps {
  form: WizardForm;
  update: (patch: Partial<WizardForm>) => void;
}

export function SettingsStep({ form, update }: SettingsStepProps) {
  const { t } = useTranslation();
  const [advanced, setAdvanced] = useState(form.port !== "");
  const nameKey = nameError(form.name);
  const portKey = portError(form.port);

  return (
    <>
      <Field label={t("wizard.name")} hint={t("wizard.nameHint")} error={nameKey && t(nameKey)}>
        <TextInput value={form.name} onChange={(e) => update({ name: e.target.value })} autoComplete="off" />
      </Field>
      <MemoryField value={form.memoryMb} onChange={(memoryMb) => update({ memoryMb })} />
      <div>
        <Button variant="ghost" size="sm" icon={advanced ? ChevronDown : ChevronRight} aria-expanded={advanced} onClick={() => setAdvanced(!advanced)}>
          {t("wizard.advanced")}
        </Button>
      </div>
      {advanced && (
        <Field label={t("wizard.port")} hint={t("wizard.portHint")} error={portKey && t(portKey)}>
          <TextInput type="number" inputMode="numeric" value={form.port} placeholder={t("wizard.portAuto")} onChange={(e) => update({ port: e.target.value })} />
        </Field>
      )}
    </>
  );
}

function MemoryField({ value, onChange }: { value: number | null; onChange: (mb: number) => void }) {
  const { t, i18n } = useTranslation();
  const system = useQuery({ queryKey: ["system"], queryFn: api.system });

  if (system.isPending) return <Spinner label={t("ui.loading")} />;
  if (system.isError) return <LoadError error={system.error} retry={() => void system.refetch()} />;

  const info = system.data;
  const suggested = info.suggested_memory_mb.max;
  const range = memoryRange(info.memory_total_bytes, info.suggested_memory_mb);
  const chosen = value ?? suggested;
  const format = (mb: number) => formatMemoryMb(mb, i18n.language);

  return (
    <>
      <Field label={t("wizard.memory")} hint={t("wizard.memoryHint")}>
        <Slider value={chosen} onChange={onChange} min={range.min} max={range.max} step={MEMORY_STEP_MB} recommended={suggested} formatValue={format} />
      </Field>
      {exceedsAvailable(chosen, info.memory_available_bytes) && (
        <Notice tone="warning">{t("wizard.memoryWarning", { free: formatBytes(info.memory_available_bytes, i18n.language) })}</Notice>
      )}
    </>
  );
}
