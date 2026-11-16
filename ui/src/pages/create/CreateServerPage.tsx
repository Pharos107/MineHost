import { useMutation, useQuery } from "@tanstack/react-query";
import { ArrowLeft, ArrowRight, Plus, X } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import { api, errorMessage, type CreateServer } from "../../api/client";
import { Button, Card, Notice, PageHeader, Stack, Steps } from "../../ui";
import { queueItems, useContentQueue } from "../catalog/queue";
import { isModLoader, nameError, portError, toRequest, type WizardForm } from "./form";
import { ImportWizard } from "./ImportWizard";
import { JobProgressView } from "./JobProgressView";
import { ModStep } from "./ModStep";
import { LoaderStep, SettingsStep, VersionStep } from "./steps";

const STEPS = ["loader", "version", "settings"] as const;
/** Server có mod có thêm bước chọn mod (spec trình duyệt mod, mục 5). */
const MOD_STEPS = ["loader", "version", "mods", "settings"] as const;
type StepKey = (typeof MOD_STEPS)[number];

/** Wizard 3 bước (4 với server có mod; spec P1, mục 2.2); bấm "Tạo" thì chuyển sang theo dõi Job. */
export function CreateServerPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [step, setStep] = useState(0);
  // "Nhập thư mục có sẵn" và "Từ modpack" ở bước 1 chuyển sang luồng riêng khi bấm Tiếp.
  const [mode, setMode] = useState<"create" | "import" | "modpack">("create");
  const [form, setForm] = useState<WizardForm>(() => ({ loader: "paper", version: "", name: t("wizard.defaultName"), memoryMb: null, port: "" }));
  const versions = useQuery({ queryKey: ["catalog", form.loader], queryFn: () => api.catalogVersions(form.loader), staleTime: 5 * 60_000 });
  const create = useMutation({ mutationFn: (body: CreateServer) => api.createServer(body) });

  const queue = useContentQueue(null);
  const [cleared, setCleared] = useState(false);

  const version = form.version || versions.data?.[0]?.id || "";
  const modded = isModLoader(form.loader);
  const steps: readonly StepKey[] = modded ? MOD_STEPS : STEPS;
  const last = steps.length - 1;
  const current = steps[step] ?? "loader";
  const update = (patch: Partial<WizardForm>) => setForm((cur) => ({ ...cur, ...patch }));
  const submit = () => create.mutate(toRequest(form, version, modded ? queueItems(queue.items) : []));
  const canContinue = current === "version" ? version !== "" : current === "settings" ? !nameError(form.name) && !portError(form.port) : true;

  // Mod đã chọn chỉ hợp loader và Minecraft lúc chọn: đổi một trong hai thì bỏ danh sách.
  const dropQueue = () => {
    if (queue.items.length === 0) return;
    queue.clear();
    setCleared(true);
  };

  if (mode === "import" && step > 0) return <ImportWizard onBack={() => setStep(0)} />;
  if (create.data) {
    const names = Object.fromEntries(queue.items.map((q) => [`${q.source}:${q.project_id}`, q.title]));
    return <JobProgressView created={create.data} contentNames={names} onRetry={submit} onBack={() => create.reset()} />;
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
            {current === "mods" && <ModStep loader={form.loader} minecraft={version} queue={queue} cleared={cleared} />}
            {current === "settings" && <SettingsStep form={form} update={update} />}
            {create.isError && <Notice tone="danger">{errorMessage(t, create.error)}</Notice>}
            <Stack direction="row" justify="between">
              <Button icon={ArrowLeft} disabled={step === 0 || create.isPending} onClick={() => setStep(step - 1)}>
                {t("wizard.back")}
              </Button>
              <Button type="submit" variant="primary" icon={step < last ? ArrowRight : Plus} disabled={!canContinue} loading={create.isPending}>
                {t(step === last ? "wizard.create" : current === "mods" && queue.items.length === 0 ? "wizard.mods.skip" : "wizard.next")}
              </Button>
            </Stack>
          </Stack>
        </form>
      </Card>
    </>
  );
}
