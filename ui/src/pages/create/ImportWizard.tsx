import { useMutation } from "@tanstack/react-query";
import { ArrowLeft, ArrowRight, FolderInput, Search, X } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import { api, errorMessage, type ImportServer, type Loader } from "../../api/client";
import { Button, Card, Facts, Field, Notice, PageHeader, Select, Stack, Steps, TextInput } from "../../ui";
import { nameError } from "./form";
import { JobProgressView } from "./JobProgressView";
import { importReady, importRequest, type ImportChoice } from "./importForm";

const STEPS = ["loader", "folder", "importName"] as const;

/**
 * Nhập thư mục server có sẵn (spec P2, mục 6.11): dán đường dẫn, MineHost nhận loại và phiên bản,
 * không nhận ra thì chọn tay, rồi chép vào MineHost. Bước 1 (chọn "Nhập") nằm ở wizard chính.
 */
export function ImportWizard({ onBack }: { onBack: () => void }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [step, setStep] = useState(1);
  const [choice, setChoice] = useState<ImportChoice>({ path: "", loader: "", minecraft: "", name: t("wizard.defaultName") });
  const inspect = useMutation({ mutationFn: (path: string) => api.inspectImport(path) });
  const run = useMutation({ mutationFn: (body: ImportServer) => api.importServer(body) });
  const update = (patch: Partial<ImportChoice>) => setChoice((current) => ({ ...current, ...patch }));
  const submit = () => {
    if (inspect.data) run.mutate(importRequest(choice, inspect.data));
  };
  const canContinue = step === 1 ? importReady(choice, inspect.data) : !nameError(choice.name);

  if (run.data) return <JobProgressView created={run.data} onRetry={submit} onBack={() => run.reset()} />;

  return (
    <>
      <PageHeader
        title={t("wizard.importTitle")}
        actions={
          <Button variant="ghost" icon={X} onClick={() => void navigate("/servers")}>
            {t("ui.cancel")}
          </Button>
        }
      />
      <Card>
        <Steps steps={STEPS.map((key) => t(`wizard.steps.${key}`))} current={step} />
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (!canContinue) return;
            if (step === 1) setStep(2);
            else submit();
          }}
        >
          <Stack gap={5}>
            {step === 1 && <FolderStep choice={choice} update={update} inspect={inspect} />}
            {step === 2 && <NameStep name={choice.name} onChange={(name) => update({ name })} />}
            {run.isError && <Notice tone="danger">{errorMessage(t, run.error)}</Notice>}
            <Stack direction="row" justify="between">
              <Button icon={ArrowLeft} disabled={run.isPending} onClick={() => (step === 1 ? onBack() : setStep(1))}>
                {t("wizard.back")}
              </Button>
              <Button type="submit" variant="primary" icon={step === 1 ? ArrowRight : FolderInput} disabled={!canContinue} loading={run.isPending}>
                {t(step === 1 ? "wizard.next" : "wizard.importSubmit")}
              </Button>
            </Stack>
          </Stack>
        </form>
      </Card>
    </>
  );
}

type Inspect = ReturnType<typeof useMutation<Awaited<ReturnType<typeof api.inspectImport>>, Error, string>>;

interface FolderStepProps {
  choice: ImportChoice;
  update: (patch: Partial<ImportChoice>) => void;
  inspect: Inspect;
}

function FolderStep({ choice, update, inspect }: FolderStepProps) {
  const { t } = useTranslation();
  const found = inspect.data;
  return (
    <>
      <Field label={t("wizard.importPath")} hint={t("wizard.importPathHint")}>
        <TextInput
          value={choice.path}
          autoComplete="off"
          onChange={(e) => {
            update({ path: e.target.value });
            inspect.reset();
          }}
        />
      </Field>
      <Stack direction="row">
        <Button icon={Search} disabled={choice.path.trim() === ""} loading={inspect.isPending} onClick={() => inspect.mutate(choice.path.trim())}>
          {t("wizard.importInspect")}
        </Button>
      </Stack>
      {inspect.isError && <Notice tone="danger">{errorMessage(t, inspect.error)}</Notice>}
      {found && (
        <Facts
          items={[
            { label: t("wizard.loader"), value: found.loader ? t(`loaders.${found.loader}`) : t("wizard.importUnknown") },
            { label: t("wizard.version"), value: found.minecraft ?? t("wizard.importUnknown") },
            { label: t("wizard.importJar"), value: found.jar ? <code>{found.jar}</code> : t("wizard.importUnknown") },
          ]}
        />
      )}
      {found?.has_mods && <Notice tone="danger">{t("errors.MODS_NOT_SUPPORTED")}</Notice>}
      {found && !found.has_mods && !found.jar && <Notice tone="danger">{t("wizard.importNoJar")}</Notice>}
      {found && !found.has_mods && found.jar && !found.loader && (
        <Field label={t("wizard.importPickLoader")}>
          <Select value={choice.loader} onChange={(e) => update({ loader: e.target.value as Loader | "" })}>
            <option value="">—</option>
            {(["paper", "purpur", "vanilla"] as const).map((loader) => (
              <option key={loader} value={loader}>
                {t(`loaders.${loader}`)}
              </option>
            ))}
          </Select>
        </Field>
      )}
      {found && !found.has_mods && found.jar && !found.minecraft && (
        <Field label={t("wizard.importPickVersion")} hint={t("wizard.importPickVersionHint")}>
          <TextInput value={choice.minecraft} autoComplete="off" onChange={(e) => update({ minecraft: e.target.value })} />
        </Field>
      )}
    </>
  );
}

function NameStep({ name, onChange }: { name: string; onChange: (name: string) => void }) {
  const { t } = useTranslation();
  const key = nameError(name);
  return (
    <>
      <Field label={t("wizard.name")} hint={t("wizard.nameHint")} error={key && t(key)}>
        <TextInput value={name} onChange={(e) => onChange(e.target.value)} autoComplete="off" />
      </Field>
      <Notice tone="info">{t("wizard.importNote")}</Notice>
    </>
  );
}
