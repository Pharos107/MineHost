import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowUpCircle, RefreshCw } from "lucide-react";
import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type Job, type Loader, type Server } from "../../api/client";
import { contentApi } from "../../api/content";
import { SERVERS_KEY } from "../../api/live";
import { Button, Card, ConfirmDialog, Facts, Field, Notice, Select, Stack, type Fact } from "../../ui";
import { JobProgress, jobError } from "./JobProgress";
import { isDowngrade, MOD_LOADERS, versionRequest, type VersionChangeResult } from "./version";

type Outcome = { tone: "success" | "danger"; text: string; title?: string };

/**
 * Mục "Phiên bản" trong Cài đặt server (spec P3, mục 3.9, 6.5): bản đang chạy, nút đổi phiên bản
 * (cũng là cập nhật build Paper, Purpur) và cập nhật modpack Modrinth. Mọi thay đổi đều có backup trước.
 */
export function VersionSection({ server }: { server: Server }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [dialog, setDialog] = useState<"version" | "modpack" | null>(null);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const loader = server.loader ?? "";
  const pack = server.modpack;
  const facts: Fact[] = [{ label: "Minecraft", value: server.minecraft ?? "—" }];
  if (MOD_LOADERS.includes(loader)) facts.push({ label: t(`loaders.${loader}`), value: server.loader_version ?? "—" });
  else if (server.build != null) facts.push({ label: t("version.build"), value: String(server.build) });
  if (pack) facts.push({ label: t("version.modpack"), value: [pack.name, pack.version_name].filter(Boolean).join(" ") });

  const finished = useCallback(
    (job: Job, success: (result: Record<string, unknown>) => string) => {
      setDialog(null);
      void queryClient.invalidateQueries({ queryKey: [...SERVERS_KEY, server.id] });
      const error = jobError(job);
      if (error) {
        const backup = typeof error.params.backup_id === "string";
        setOutcome({ tone: "danger", text: errorMessage(t, error), title: backup ? t("version.failedWithBackup") : undefined });
      } else if (job.state === "succeeded") {
        setOutcome({ tone: "success", text: success((job.result ?? {}) as Record<string, unknown>) });
      }
    },
    [queryClient, server.id, t],
  );

  return (
    <Card title={t("version.title")}>
      <Facts items={facts} />
      <Stack direction="row" gap={2} wrap>
        <Button icon={RefreshCw} onClick={() => setDialog("version")}>
          {t("version.change")}
        </Button>
        {pack?.source === "modrinth" && pack.project_id && (
          <Button icon={ArrowUpCircle} onClick={() => setDialog("modpack")}>
            {t("version.updateModpack")}
          </Button>
        )}
      </Stack>
      {outcome && (
        <Notice tone={outcome.tone} title={outcome.title}>
          {outcome.text}
        </Notice>
      )}
      {dialog === "version" && (
        <VersionDialog
          server={server}
          onCancel={() => setDialog(null)}
          onFinish={(job) =>
            finished(job, (result) => {
              const res = result as unknown as VersionChangeResult;
              return res.changed ? t("version.changed", { minecraft: res.minecraft }) : t("version.upToDate");
            })
          }
        />
      )}
      {dialog === "modpack" && pack?.project_id && (
        <ModpackDialog
          server={server}
          projectId={pack.project_id}
          currentVersion={pack.version_id ?? null}
          onCancel={() => setDialog(null)}
          onFinish={(job) => finished(job, (result) => t("version.modpackUpdated", { removed: result.removed, kept: result.kept }))}
        />
      )}
    </Card>
  );
}

interface DialogProps {
  server: Server;
  onCancel: () => void;
  onFinish: (job: Job) => void;
}

function VersionDialog({ server, onCancel, onFinish }: DialogProps) {
  const { t } = useTranslation();
  const loader = (server.loader ?? "paper") as Loader;
  const current = server.minecraft ?? "";
  const [minecraft, setMinecraft] = useState(current);
  const [loaderVersion, setLoaderVersion] = useState("");
  const [jobId, setJobId] = useState<number | null>(null);
  const versions = useQuery({ queryKey: ["catalog", loader], queryFn: () => api.catalogVersions(loader) });
  const modLoader = MOD_LOADERS.includes(loader);
  const loaderVersions = useQuery({
    queryKey: ["catalog", loader, minecraft, "loader-versions"],
    queryFn: () => api.loaderVersions(loader, minecraft),
    enabled: modLoader && minecraft !== "",
  });
  const start = useMutation({
    mutationFn: () => api.changeVersion(server.id, versionRequest(current, minecraft, loaderVersion)),
    onSuccess: (res) => setJobId(res.job_id),
  });
  const downgrade = isDowngrade(versions.data ?? [], current, minecraft);
  const choices = versions.data?.map((v) => v.id) ?? [current];
  return (
    <ConfirmDialog
      open
      title={t("version.changeTitle")}
      confirmLabel={t("version.confirm")}
      tone={downgrade ? "danger" : "primary"}
      requireAck={downgrade ? t("version.downgradeAck") : undefined}
      loading={start.isPending || jobId !== null}
      onCancel={() => {
        if (jobId === null) onCancel();
      }}
      onConfirm={() => start.mutate()}
    >
      <p>{t("version.changeBody")}</p>
      <Field label="Minecraft">
        <Select value={minecraft} onChange={(e) => setMinecraft(e.target.value)}>
          {(choices.includes(current) ? choices : [current, ...choices]).map((id) => (
            <option key={id} value={id}>
              {id === current ? t("version.current", { version: id }) : id}
            </option>
          ))}
        </Select>
      </Field>
      {modLoader && (
        <Field label={t(`loaders.${loader}`)}>
          <Select value={loaderVersion} onChange={(e) => setLoaderVersion(e.target.value)}>
            <option value="">{t("version.recommended")}</option>
            {(loaderVersions.data ?? []).map((v) => (
              <option key={v.version} value={v.version}>
                {v.version === server.loader_version ? t("version.current", { version: v.version }) : v.version}
              </option>
            ))}
          </Select>
        </Field>
      )}
      {downgrade && <Notice tone="danger">{t("version.downgradeWarning")}</Notice>}
      {versions.isError && <Notice tone="danger">{errorMessage(t, versions.error)}</Notice>}
      {start.isError && <Notice tone="danger">{errorMessage(t, start.error)}</Notice>}
      {jobId !== null && <JobProgress jobId={jobId} onFinish={onFinish} />}
    </ConfirmDialog>
  );
}

function ModpackDialog({ server, projectId, currentVersion, onCancel, onFinish }: DialogProps & { projectId: string; currentVersion: string | null }) {
  const { t } = useTranslation();
  const [chosen, setChosen] = useState<string | null>(null);
  const [jobId, setJobId] = useState<number | null>(null);
  const versions = useQuery({ queryKey: ["modpack-versions", projectId], queryFn: () => contentApi.versions("modrinth", projectId) });
  const selected = chosen ?? versions.data?.[0]?.id ?? "";
  const start = useMutation({
    mutationFn: () => api.updateModpack(server.id, selected),
    onSuccess: (res) => setJobId(res.job_id),
  });
  return (
    <ConfirmDialog
      open
      title={t("version.modpackTitle")}
      confirmLabel={t("version.confirm")}
      tone="primary"
      loading={start.isPending || jobId !== null}
      onCancel={() => {
        if (jobId === null) onCancel();
      }}
      onConfirm={() => {
        if (selected) start.mutate();
      }}
    >
      <p>{t("version.modpackBody")}</p>
      <Field label={t("version.modpackVersion")}>
        <Select value={selected} onChange={(e) => setChosen(e.target.value)}>
          {(versions.data ?? []).map((v) => (
            <option key={v.id} value={v.id}>
              {v.id === currentVersion ? t("version.current", { version: v.version_number ?? v.name }) : (v.version_number ?? v.name)}
            </option>
          ))}
        </Select>
      </Field>
      {versions.isError && <Notice tone="danger">{errorMessage(t, versions.error)}</Notice>}
      {start.isError && <Notice tone="danger">{errorMessage(t, start.error)}</Notice>}
      {jobId !== null && <JobProgress jobId={jobId} onFinish={onFinish} />}
    </ConfirmDialog>
  );
}
