import { useMutation, useQuery } from "@tanstack/react-query";
import { ArrowLeft, ArrowRight, Plus, X } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import { api, errorMessage, type CreateServer } from "../../api/client";
import { Button, Card, Notice, PageHeader, Stack, Steps } from "../../ui";
import { nameError, portError, toRequest, type WizardForm } from "./form";
import { JobProgressView } from "./JobProgressView";
import { LoaderStep, SettingsStep, VersionStep } from "./steps";

const STEPS = ["loader", "version", "settings"] as const;
const LAST = STEPS.length - 1;

/** Wizard 3 bước (spec P1, mục 2.2); bấm "Tạo" thì chuyển sang theo dõi Job. */
export function CreateServerPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [step, setStep] = useState(0);
  const [form, setForm] = useState<WizardForm>(() => ({ loader: "paper", version: "", name: t("wizard.defaultName"), memoryMb: null, port: "" }));
  const versions = useQuery({ queryKey: ["catalog", "paper"], queryFn: api.paperVersions, staleTime: 5 * 60_000 });
  const create = useMutation({ mutationFn: (body: CreateServer) => api.createServer(body) });

  const version = form.version || versions.data?.[0]?.id || "";
  const update = (patch: Partial<WizardForm>) => setForm((current) => ({ ...current, ...patch }));
  const submit = () => create.mutate(toRequest(form, version));
  const canContinue = [true, version !== "", !nameError(form.name) && !portError(form.port)][step];

  if (create.data) {
    return <JobProgressView created={create.data} onRetry={submit} onBack={() => create.reset()} />;
  }

  return (
    <>
      <PageHeader
        title={t("wizard.title")}
        actions={
          <Button variant="ghost" icon={X} onClick={() => void navigate("/")}>
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
            if (step < LAST) setStep(step + 1);
            else submit();
          }}
        >
          <Stack gap={5}>
            {step === 0 && <LoaderStep value={form.loader} onChange={(loader) => update({ loader })} />}
            {step === 1 && <VersionStep versions={versions} value={version} onChange={(v) => update({ version: v })} />}
            {step === 2 && <SettingsStep form={form} update={update} />}
            {create.isError && <Notice tone="danger">{errorMessage(t, create.error)}</Notice>}
            <Stack direction="row" justify="between">
              <Button icon={ArrowLeft} disabled={step === 0 || create.isPending} onClick={() => setStep(step - 1)}>
                {t("wizard.back")}
              </Button>
              <Button type="submit" variant="primary" icon={step < LAST ? ArrowRight : Plus} disabled={!canContinue} loading={create.isPending}>
                {t(step < LAST ? "wizard.next" : "wizard.create")}
              </Button>
            </Stack>
          </Stack>
        </form>
      </Card>
    </>
  );
}
