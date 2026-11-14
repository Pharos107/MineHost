import { useInfiniteQuery, useMutation, useQuery } from "@tanstack/react-query";
import { ArrowLeft, Package, Plus, RotateCw, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import { errorMessage, type Created } from "../../api/client";
import { contentApi, modpackApi, type CreateModpackBody, type SearchHit } from "../../api/content";
import { CURSEFORGE_KEY, settingsApi } from "../../api/settings";
import { formatBytes } from "../../format";
import { memoryFor } from "../../memory";
import {
  Button,
  Card,
  EmptyState,
  Facts,
  Field,
  FileDrop,
  Notice,
  PageHeader,
  ProjectCard,
  ProjectCardGrid,
  SearchInput,
  SegmentedControl,
  Select,
  Spinner,
  Stack,
} from "../../ui";
import { hitLoaders, hitVersion, versionOptionLabel } from "../server/content/content";
import { nameError, portError } from "./form";
import { ModpackJobView } from "./ModpackJobView";
import { SettingsStep, type ServerSettingsValues } from "./steps";

type PackTarget = { kind: "catalog"; hit: SearchHit } | { kind: "file"; file: File };

function nameFromFile(fileName: string, fallback: string): string {
  const base = fileName.replace(/\.(mrpack|zip)$/i, "").trim();
  return (base || fallback).slice(0, 64);
}

function buildUploadForm(file: File, form: ServerSettingsValues): FormData {
  const body = new FormData();
  body.append("file", file, file.name);
  body.append("name", form.name.trim());
  if (form.memoryMb !== null) {
    body.append("memory_mb", JSON.stringify(memoryFor(form.memoryMb)));
  }
  if (form.port.trim() !== "") {
    body.append("port", form.port.trim());
  }
  return body;
}

/** Trang tìm, tải lên file `.mrpack` và tạo server từ modpack (`/new/modpack`, spec P3, mục 6.2). */
export function CreateModpackPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [target, setTarget] = useState<PackTarget | null>(null);
  const [versionId, setVersionId] = useState("");
  const [form, setForm] = useState<ServerSettingsValues>(() => ({
    name: t("wizard.defaultName"),
    memoryMb: null,
    port: "",
  }));

  const createPack = useMutation({
    mutationFn: (body: CreateModpackBody) => modpackApi.create(body),
  });
  const uploadPack = useMutation({
    mutationFn: (body: FormData) => modpackApi.upload(body),
  });

  const activeMutation = target?.kind === "file" ? uploadPack : createPack;
  const created: Created | undefined = activeMutation.data;

  function chooseHit(hit: SearchHit) {
    createPack.reset();
    uploadPack.reset();
    setTarget({ kind: "catalog", hit });
    setVersionId("");
    setForm({ name: hit.title.trim().slice(0, 64) || t("wizard.defaultName"), memoryMb: null, port: "" });
  }

  function chooseFile(file: File) {
    createPack.reset();
    uploadPack.reset();
    setTarget({ kind: "file", file });
    setVersionId("");
    setForm({ name: nameFromFile(file.name, t("wizard.defaultName")), memoryMb: null, port: "" });
  }

  function submit(chosenVersionId: string) {
    if (!target) return;
    if (target.kind === "file") {
      uploadPack.mutate(buildUploadForm(target.file, form));
      return;
    }
    createPack.mutate({
      source: target.hit.source,
      project_id: target.hit.project_id,
      version_id: chosenVersionId || null,
      name: form.name.trim(),
      memory_mb: form.memoryMb === null ? null : memoryFor(form.memoryMb),
      port: form.port.trim() === "" ? null : Number(form.port),
    });
  }

  if (created) {
    return (
      <ModpackJobView
        created={created}
        onRetry={() => submit(versionId)}
        onBack={() => activeMutation.reset()}
      />
    );
  }

  return (
    <>
      <PageHeader
        title={t("modpack.title")}
        description={t("modpack.description")}
        actions={
          <Button variant="ghost" icon={X} onClick={() => void navigate("/servers")}>
            {t("ui.cancel")}
          </Button>
        }
      />
      {target ? (
        <ConfigurePackCard
          target={target}
          versionId={versionId}
          onVersionChange={setVersionId}
          form={form}
          update={(patch) => setForm((cur) => ({ ...cur, ...patch }))}
          pending={activeMutation.isPending}
          error={activeMutation.isError ? activeMutation.error : null}
          onBack={() => {
            activeMutation.reset();
            setTarget(null);
          }}
          onSubmit={submit}
        />
      ) : (
        <Stack gap={5}>
          <Card title={t("modpack.dropTitle")}>
            <FileDrop
              label={t("modpack.dropLabel")}
              hint={t("modpack.dropHint")}
              accept=".mrpack,.zip"
              onFiles={(files) => {
                const file = files[0];
                if (file) chooseFile(file);
              }}
            />
          </Card>
          <ModpackSearchCard onSelect={chooseHit} />
        </Stack>
      )}
    </>
  );
}

function ModpackSearchCard({ onSelect }: { onSelect: (hit: SearchHit) => void }) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [submitted, setSubmitted] = useState("");

  useEffect(() => {
    const trimmed = query.trim();
    const timer = window.setTimeout(() => setSubmitted(trimmed), trimmed === "" ? 0 : 300);
    return () => window.clearTimeout(timer);
  }, [query]);

  const cfSettings = useQuery({
    queryKey: CURSEFORGE_KEY,
    queryFn: settingsApi.curseforge,
  });
  const cfReason = cfSettings.data?.available ? t("modpack.curseforgeSoon") : t("modpack.curseforgeNoKey");

  const search = useInfiniteQuery({
    queryKey: ["modpack-search", submitted],
    queryFn: ({ pageParam }) => contentApi.search({ source: "modrinth", kind: "modpack", q: submitted, page: pageParam }),
    initialPageParam: 1,
    getNextPageParam: (last) => (last.page * last.page_size < last.total ? last.page + 1 : undefined),
  });
  const hits = search.data?.pages.flatMap((page) => page.hits) ?? [];

  return (
    <Card title={t("modpack.searchTitle")}>
      <Stack gap={4}>
        <SegmentedControl
          label={t("content.addDialog.sourceLabel")}
          value="modrinth"
          onChange={() => {}}
          options={[
            { value: "modrinth", label: t("content.addDialog.sourceModrinth") },
            {
              value: "curseforge",
              label: t("content.addDialog.sourceCurseforge"),
              disabled: true,
              title: cfReason,
            },
          ]}
        />
        {!cfSettings.isPending && <Notice tone="info">{cfReason}</Notice>}
        <form
          onSubmit={(e) => {
            e.preventDefault();
            setSubmitted(query.trim());
          }}
        >
          <SearchInput value={query} onChange={setQuery} label={t("modpack.searchPlaceholder")} />
        </form>
        {search.isPending ? (
          <Spinner label={t("ui.loading")} />
        ) : search.isError ? (
          <Notice
            tone="danger"
            action={
              <Button size="sm" icon={RotateCw} onClick={() => void search.refetch()}>
                {t("common.retry")}
              </Button>
            }
          >
            {errorMessage(t, search.error)}
          </Notice>
        ) : hits.length === 0 ? (
          <EmptyState icon={Package} title={t("modpack.noResults")} description={t("modpack.noResultsHint")} />
        ) : (
          <Stack gap={3}>
            <ProjectCardGrid>
              {hits.map((hit) => (
                <ProjectCard
                  key={`${hit.source}:${hit.project_id}`}
                  title={hit.title}
                  description={hit.description}
                  iconUrl={hit.icon_url}
                  downloads={hit.downloads}
                  loaders={hitLoaders(hit.categories)}
                  version={hitVersion(hit.game_versions)}
                  onSelect={() => onSelect(hit)}
                />
              ))}
            </ProjectCardGrid>
            {search.hasNextPage && (
              <div>
                <Button variant="secondary" loading={search.isFetchingNextPage} onClick={() => void search.fetchNextPage()}>
                  {t("content.addDialog.loadMore")}
                </Button>
              </div>
            )}
          </Stack>
        )}
      </Stack>
    </Card>
  );
}

interface ConfigurePackCardProps {
  target: PackTarget;
  versionId: string;
  onVersionChange: (versionId: string) => void;
  form: ServerSettingsValues;
  update: (patch: Partial<ServerSettingsValues>) => void;
  pending: boolean;
  error: unknown;
  onBack: () => void;
  onSubmit: (chosenVersionId: string) => void;
}

function ConfigurePackCard({
  target,
  versionId,
  onVersionChange,
  form,
  update,
  pending,
  error,
  onBack,
  onSubmit,
}: ConfigurePackCardProps) {
  const { t, i18n } = useTranslation();
  const isCatalog = target.kind === "catalog";
  const versions = useQuery({
    queryKey: ["modpack-versions", isCatalog ? target.hit.source : "", isCatalog ? target.hit.project_id : ""],
    queryFn: () =>
      isCatalog ? contentApi.versions(target.hit.source, target.hit.project_id) : Promise.resolve([]),
    enabled: isCatalog,
  });

  const versionList = versions.data ?? [];
  const chosenVersionId = versionId || versionList[0]?.id || "";
  const versionReady = !isCatalog || (versions.isSuccess && chosenVersionId !== "");
  const canSubmit = versionReady && !nameError(form.name) && !portError(form.port);

  return (
    <Card>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (canSubmit) onSubmit(chosenVersionId);
        }}
      >
        <Stack gap={5}>
          {isCatalog ? (
            <>
              <ProjectCard
                title={target.hit.title}
                description={target.hit.description}
                iconUrl={target.hit.icon_url}
                downloads={target.hit.downloads}
                loaders={hitLoaders(target.hit.categories)}
                version={hitVersion(target.hit.game_versions)}
              />
              {versions.isPending ? (
                <Spinner label={t("ui.loading")} />
              ) : versions.isError ? (
                <Notice tone="danger">{errorMessage(t, versions.error)}</Notice>
              ) : versionList.length === 0 ? (
                <Notice tone="warning">{t("errors.CONTENT_NOT_FOUND")}</Notice>
              ) : (
                <Field label={t("modpack.version")}>
                  <Select value={chosenVersionId} disabled={pending} onChange={(e) => onVersionChange(e.target.value)}>
                    {versionList.map((ver, index) => {
                      const label = versionOptionLabel(ver);
                      return (
                        <option key={ver.id} value={ver.id}>
                          {index === 0 ? t("wizard.latest", { version: label }) : label}
                        </option>
                      );
                    })}
                  </Select>
                </Field>
              )}
            </>
          ) : (
            <Facts
              items={[
                { label: t("modpack.fileLabel"), value: target.file.name },
                { label: t("modpack.fileSize"), value: formatBytes(target.file.size, i18n.language) },
              ]}
            />
          )}
          <SettingsStep form={form} update={update} modpack />
          {error !== null && <Notice tone="danger">{errorMessage(t, error)}</Notice>}
          <Stack direction="row" justify="between">
            <Button icon={ArrowLeft} disabled={pending} onClick={onBack}>
              {t("wizard.back")}
            </Button>
            <Button type="submit" variant="primary" icon={Plus} disabled={!canSubmit} loading={pending}>
              {t("wizard.create")}
            </Button>
          </Stack>
        </Stack>
      </form>
    </Card>
  );
}
