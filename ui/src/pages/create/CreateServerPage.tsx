import { useMutation, useQuery } from "@tanstack/react-query";
import { ArrowLeft, ArrowRight, Plus, X } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import { api, errorMessage, type CreateServer } from "../../api/client";
import { Button, Card, Notice, PageHeader, Stack, Steps, type Fact } from "../../ui";
import { queueItems, useContentQueue } from "../catalog/queue";
import {
  contentKindForLoader,
  DEFAULT_GAME_SETTINGS,
  hasGameSettingsError,
  isModLoader,
  isPluginLoader,
  nameError,
  portError,
  toRequest,
  type WizardForm,
} from "./form";
import { ImportWizard } from "./ImportWizard";
import { JobProgressView } from "./JobProgressView";
import { ModStep } from "./ModStep";
import { LoaderStep, SettingsStep, VersionStep } from "./steps";

const STEPS = ["loader", "version", "settings"] as const;
/** Server có mod có thêm bước chọn mod (spec trình duyệt mod, mục 5). */
const MOD_STEPS = ["loader", "version", "mods", "settings"] as const;
/** Server hỗ trợ plugin (Paper, Purpur) có thêm bước chọn plugin. */
const PLUGIN_STEPS = ["loader", "version", "plugins", "settings"] as const;
const MODPACK_STEPS = ["loader", "modpack", "settings"] as const;
const IMPORT_STEPS = ["loader", "folder", "importName"] as const;
type StepKey =
  | (typeof MOD_STEPS)[number]
  | (typeof PLUGIN_STEPS)[number]
  | (typeof MODPACK_STEPS)[number]
  | (typeof IMPORT_STEPS)[number];

/** Wizard tạo server (3 bước với Vanilla, 4 bước với mod/plugin loader); bấm "Tạo" thì theo dõi Job. */
export function CreateServerPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [step, setStep] = useState(0);
  // "Nhập thư mục có sẵn" và "Từ modpack" ở bước 1 chuyển sang luồng riêng khi bấm Tiếp.
  const [mode, setMode] = useState<"create" | "import" | "modpack">("create");
  const [form, setForm] = useState<WizardForm>(() => ({
    loader: "paper",
    version: "",
    name: t("wizard.defaultName"),
    memoryMb: null,
    port: "",
    game: DEFAULT_GAME_SETTINGS,
  }));
  const versions = useQuery({ queryKey: ["catalog", form.loader], queryFn: () => api.catalogVersions(form.loader), staleTime: 5 * 60_000 });
  const create = useMutation({ mutationFn: (body: CreateServer) => api.createServer(body) });

  const queue = useContentQueue(null);
  const [cleared, setCleared] = useState(false);

  const version = form.version || versions.data?.[0]?.id || "";
  const contentKind = contentKindForLoader(form.loader);
  const steps: readonly StepKey[] =
    mode === "modpack"
      ? MODPACK_STEPS
      : mode === "import"
        ? IMPORT_STEPS
        : isModLoader(form.loader)
          ? MOD_STEPS
          : isPluginLoader(form.loader)
            ? PLUGIN_STEPS
            : STEPS;
  const last = steps.length - 1;
  const current = steps[step] ?? "loader";
  const summary: Fact[] = [
    { label: t("wizard.loader"), value: t(`loaders.${form.loader}`) },
    {
      label: t("wizard.version"),
      value: form.loaderVersion ? `${version} (${form.loaderVersion})` : version,
    },
    ...(contentKind === "mod"
      ? [{ label: t("wizard.summaryMods"), value: String(queue.items.length) }]
      : contentKind === "plugin"
        ? [{ label: t("wizard.summaryPlugins"), value: String(queue.items.length) }]
        : []),
  ];
  const update = (patch: Partial<WizardForm>) => setForm((cur) => ({ ...cur, ...patch }));
  const submit = () => create.mutate(toRequest(form, version, contentKind ? queueItems(queue.items) : []));
  const canContinue =
    current === "version"
      ? version !== ""
      : current === "settings"
        ? !nameError(form.name) && !portError(form.port) && !hasGameSettingsError(form.game)
        : true;
  const isContentStep = current === "mods" || current === "plugins";

  // Mod đã chọn chỉ hợp loader và Minecraft lúc chọn: đổi một trong hai thì bỏ danh sách.
  const dropQueue = () => {
    if (queue.items.length === 0) return;
    queue.clear();
    setCleared(true);
  };

  if (mode === "import" && step > 0) return <ImportWizard onBack={() => setStep(0)} />;
  if (create.data) {
    const names = Object.fromEntries(queue.items.map((q) => [`${q.source}:${q.project_id}`, q.title]));
    return (
      <JobProgressView
        created={create.data}
        contentKind={contentKind}
        contentNames={names}
        onRetry={submit}
        onBack={() => create.reset()}
      />
    );
  }

  return (
    <>
      <PageHeader
        title={t("wizard.title")}
        actions={
          <Button variant="ghost" icon={X} onClick={() => void navigate("/servers")}>
            {t("ui.cancel")}
          </Button>
        }
      />
      <Card>
        <Steps steps={steps.map((key) => t(`wizard.steps.${key}`))} current={step} />
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (!canContinue) return;
            if (step === 0 && mode === "modpack") {
              void navigate("/new/modpack");
              return;
            }
            if (step < last) setStep(step + 1);
            else submit();
          }}
        >
          <Stack gap={5}>
            {/* Mỗi loại có danh sách phiên bản riêng: đổi loại thì chọn lại bản mới nhất của loại đó. */}
            {current === "loader" && (
              <LoaderStep
                value={mode === "create" ? form.loader : mode}
                onChange={(choice) => {
                  if (choice === "import" || choice === "modpack") {
                    setMode(choice);
                    return;
                  }
                  setMode("create");
                  if (choice !== form.loader) dropQueue();
                  if (!contentKindForLoader(choice)) setCleared(false);
                  update({ loader: choice, version: "", loaderVersion: "" });
                }}
              />
            )}
            {current === "version" && (
              <VersionStep
                versions={versions}
                value={version}
                onChange={(v) => {
                  if (v !== version) dropQueue();
                  update({ version: v, loaderVersion: "" });
                }}
                loader={form.loader}
                loaderVersion={form.loaderVersion}
                onLoaderVersionChange={(loaderVersion) => update({ loaderVersion })}
              />
            )}
            {isContentStep && <ModStep loader={form.loader} minecraft={version} queue={queue} cleared={cleared} onAdded={() => setCleared(false)} />}
            {current === "settings" && <SettingsStep form={form} update={update} summary={summary} />}
            {create.isError && <Notice tone="danger">{errorMessage(t, create.error)}</Notice>}
            <Stack direction="row" justify="between">
              <Button icon={ArrowLeft} disabled={step === 0 || create.isPending} onClick={() => setStep(step - 1)}>
                {t("wizard.back")}
              </Button>
              <Button type="submit" variant="primary" icon={step < last ? ArrowRight : Plus} disabled={!canContinue} loading={create.isPending}>
                {t(step === last ? "wizard.create" : isContentStep && queue.items.length === 0 ? "wizard.mods.skip" : "wizard.next")}
              </Button>
            </Stack>
          </Stack>
        </form>
      </Card>
    </>
  );
}
