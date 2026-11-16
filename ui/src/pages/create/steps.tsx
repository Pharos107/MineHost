import type { UseQueryResult } from "@tanstack/react-query";
import { useQuery } from "@tanstack/react-query";
import { Blocks, ChevronDown, ChevronRight, FolderInput, Package, RotateCw, Server } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type CatalogVersion, type Loader } from "../../api/client";
import { formatBytes, formatMemoryMb } from "../../format";
import { exceedsAvailable, MEMORY_STEP_MB, memoryRange } from "../../memory";
import { Badge, Button, Card, ChoiceGroup, Columns, Facts, Field, Notice, Select, Slider, Spinner, Stack, Switch, TextInput, type Fact } from "../../ui";
import {
  DEFAULT_GAME_SETTINGS,
  hasAdvancedChanges,
  isModLoader,
  maxPlayersError,
  MOD_LOADERS,
  nameError,
  portError,
  spawnProtectionError,
  STANDARD_LOADERS,
  viewDistanceError,
  type GameSettingsValues,
  type WizardForm,
} from "./form";

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
export function LoaderStep({
  value,
  onChange,
}: {
  value: Loader | "import" | "modpack";
  onChange: (value: Loader | "import" | "modpack") => void;
}) {
  const { t } = useTranslation();
  const kind = value === "import" ? "import" : value === "modpack" ? "modpack" : isModLoader(value) ? "modded" : "standard";
  const loaders = kind === "modded" ? MOD_LOADERS : STANDARD_LOADERS;

  return (
    <Stack gap={5}>
      <ChoiceGroup
        label={t("wizard.kind")}
        value={kind}
        onChange={(next) => {
          if (next === "standard") onChange("paper");
          else if (next === "modded") onChange("fabric");
          else if (next === "modpack") onChange("modpack");
          else if (next === "import") onChange("import");
        }}
        options={[
          { value: "standard", label: t("wizard.kindStandard"), description: t("wizard.kindStandardDescription"), icon: Server },
          { value: "modded", label: t("wizard.kindModded"), description: t("wizard.kindModdedDescription"), icon: Blocks },
          { value: "modpack", label: t("wizard.kindModpack"), description: t("wizard.kindModpackDescription"), icon: Package },
          { value: "import", label: t("wizard.importLabel"), description: t("wizard.importDescription"), icon: FolderInput },
        ]}
      />
      {kind === "modpack" || kind === "import" ? (
        <Card variant="subtle">
          <Stack direction="row" align="center" gap={2}>
            {kind === "modpack" ? <Package size={18} aria-hidden /> : <FolderInput size={18} aria-hidden />}
            <span>{t(kind === "modpack" ? "wizard.modpackStepHint" : "wizard.importStepHint")}</span>
          </Stack>
        </Card>
      ) : (
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

export type ServerSettingsValues = Pick<WizardForm, "name" | "memoryMb" | "port" | "game">;

interface SettingsStepProps {
  form: ServerSettingsValues;
  update: (patch: Partial<ServerSettingsValues>) => void;
  modpack?: boolean;
  summary?: Fact[];
}

export function SettingsStep({ form, update, modpack = false, summary }: SettingsStepProps) {
  const { t } = useTranslation();
  const game = form.game ?? DEFAULT_GAME_SETTINGS;
  const [advanced, setAdvanced] = useState(() => hasAdvancedChanges(form.port, game));
  const nameKey = nameError(form.name);
  const portKey = portError(form.port);
  const hasAdvError = Boolean(
    portKey || viewDistanceError(game.viewDistance) || spawnProtectionError(game.spawnProtection),
  );
  const showAdvanced = advanced || hasAdvError;
  const setGame = (patch: Partial<GameSettingsValues>) => update({ game: { ...game, ...patch } });

  return (
    <>
      {summary && summary.length > 0 && (
        <Card variant="subtle">
          <Facts items={summary} />
        </Card>
      )}
      <Field label={t("wizard.name")} hint={t("wizard.nameHint")} error={nameKey && t(nameKey)}>
        <TextInput value={form.name} onChange={(e) => update({ name: e.target.value })} autoComplete="off" />
      </Field>
      <MemoryField value={form.memoryMb} onChange={(memoryMb) => update({ memoryMb })} modpack={modpack} />
      <GameOptionsCard game={game} setGame={setGame} />
      <div>
        <Button
          variant="ghost"
          size="sm"
          icon={showAdvanced ? ChevronDown : ChevronRight}
          aria-expanded={showAdvanced}
          onClick={() => setAdvanced(hasAdvError ? true : !advanced)}
        >
          {t("wizard.advanced")}
        </Button>
      </div>
      {showAdvanced && <AdvancedPropertiesFields port={form.port} portKey={portKey} onPortChange={(port) => update({ port })} game={game} setGame={setGame} />}
    </>
  );
}

const GAMEMODES = ["survival", "creative", "adventure", "spectator"] as const;
const DIFFICULTIES = ["peaceful", "easy", "normal", "hard"] as const;

function GameOptionsCard({ game, setGame }: { game: GameSettingsValues; setGame: (patch: Partial<GameSettingsValues>) => void }) {
  const { t } = useTranslation();
  const maxPlayersKey = maxPlayersError(game.maxPlayers);

  return (
    <Card variant="subtle" title={t("wizard.gameSettingsTitle")} description={t("wizard.gameSettingsDescription")}>
      <Columns count={2}>
        <Field label={t("game.fields.gamemode")}>
          <Select value={game.gamemode} onChange={(e) => setGame({ gamemode: e.target.value })}>
            {GAMEMODES.map((mode) => (
              <option key={mode} value={mode}>
                {t(`game.options.${mode}`)}
              </option>
            ))}
          </Select>
        </Field>
        <Field label={t("game.fields.difficulty")}>
          <Select value={game.difficulty} onChange={(e) => setGame({ difficulty: e.target.value })}>
            {DIFFICULTIES.map((diff) => (
              <option key={diff} value={diff}>
                {t(`game.options.${diff}`)}
              </option>
            ))}
          </Select>
        </Field>
      </Columns>
      <Columns count={2}>
        <Field label={t("game.fields.max-players")} error={maxPlayersKey && t(maxPlayersKey)}>
          <TextInput
            type="number"
            inputMode="numeric"
            min={1}
            max={100000}
            value={game.maxPlayers}
            onChange={(e) => setGame({ maxPlayers: e.target.value })}
          />
        </Field>
        <Field label={t("game.fields.motd")} hint={t("game.hints.motd")}>
          <TextInput
            value={game.motd}
            placeholder={t("wizard.motdPlaceholder")}
            onChange={(e) => setGame({ motd: e.target.value })}
            autoComplete="off"
          />
        </Field>
      </Columns>
      <Switch
        label={t("game.fields.online-mode")}
        description={t("game.hints.online-mode")}
        checked={game.onlineMode}
        onChange={(onlineMode) => setGame({ onlineMode })}
      />
      <Switch
        label={t("game.fields.white-list")}
        description={t("game.hints.white-list")}
        checked={game.whiteList}
        onChange={(whiteList) => setGame({ whiteList })}
      />
      <Switch label={t("game.fields.pvp")} checked={game.pvp} onChange={(pvp) => setGame({ pvp })} />
    </Card>
  );
}

function AdvancedPropertiesFields({
  port,
  portKey,
  onPortChange,
  game,
  setGame,
}: {
  port: string;
  portKey?: string;
  onPortChange: (port: string) => void;
  game: GameSettingsValues;
  setGame: (patch: Partial<GameSettingsValues>) => void;
}) {
  const { t } = useTranslation();
  const viewDistKey = viewDistanceError(game.viewDistance);
  const spawnProtKey = spawnProtectionError(game.spawnProtection);

  return (
    <Stack gap={4}>
      <Columns count={2}>
        <Field label={t("wizard.port")} hint={t("wizard.portHint")} error={portKey && t(portKey)}>
          <TextInput type="number" inputMode="numeric" value={port} placeholder={t("wizard.portAuto")} onChange={(e) => onPortChange(e.target.value)} />
        </Field>
        <Field label={t("wizard.viewDistance")} hint={t("wizard.viewDistanceHint")} error={viewDistKey && t(viewDistKey)}>
          <TextInput
            type="number"
            inputMode="numeric"
            min={2}
            max={32}
            value={game.viewDistance}
            onChange={(e) => setGame({ viewDistance: e.target.value })}
          />
        </Field>
      </Columns>
      <Columns count={2}>
        <Field label={t("wizard.spawnProtection")} hint={t("wizard.spawnProtectionHint")} error={spawnProtKey && t(spawnProtKey)}>
          <TextInput
            type="number"
            inputMode="numeric"
            min={0}
            max={1000}
            value={game.spawnProtection}
            onChange={(e) => setGame({ spawnProtection: e.target.value })}
          />
        </Field>
        <Field label={t("wizard.levelSeed")} hint={t("wizard.levelSeedHint")}>
          <TextInput
            value={game.levelSeed}
            placeholder={t("wizard.seedPlaceholder")}
            onChange={(e) => setGame({ levelSeed: e.target.value })}
            autoComplete="off"
          />
        </Field>
      </Columns>
      <Switch
        label={t("wizard.commandBlocks")}
        description={t("wizard.commandBlocksHint")}
        checked={game.enableCommandBlocks}
        onChange={(enableCommandBlocks) => setGame({ enableCommandBlocks })}
      />
      <Switch
        label={t("wizard.allowFlight")}
        description={t("wizard.allowFlightHint")}
        checked={game.allowFlight}
        onChange={(allowFlight) => setGame({ allowFlight })}
      />
    </Stack>
  );
}

function MemoryField({
  value,
  onChange,
  modpack = false,
}: {
  value: number | null;
  onChange: (mb: number) => void;
  modpack?: boolean;
}) {
  const { t, i18n } = useTranslation();
  const system = useQuery({ queryKey: ["system"], queryFn: api.system });

  if (system.isPending) return <Spinner label={t("ui.loading")} />;
  if (system.isError) return <LoadError error={system.error} retry={() => void system.refetch()} />;

  const info = system.data;
  const suggestedLimits = modpack ? info.suggested_modpack_memory_mb : info.suggested_memory_mb;
  const suggested = suggestedLimits.max;
  const range = memoryRange(info.memory_total_bytes, suggestedLimits);
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
