import { useInfiniteQuery, useMutation, useQuery } from "@tanstack/react-query";
import { ArrowLeft, Package, RotateCw } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, ApiError, errorMessage, type Server } from "../../../api/client";
import {
  contentApi,
  type ContentKind,
  type ContentPlan,
  type PlannedContent,
  type SearchHit,
} from "../../../api/content";
import { useJob } from "../../../api/live";
import { formatBytes } from "../../../format";
import { jobPercent, jobStepLabel } from "../../../jobs";
import {
  Badge,
  Button,
  Card,
  Dialog,
  DialogBody,
  DialogFooter,
  EmptyState,
  Facts,
  Field,
  Notice,
  ProgressBar,
  ProjectCard,
  SearchInput,
  Select,
  Spinner,
  Stack,
} from "../../../ui";
import { hitLoaders, hitVersion, searchKindFor, versionOptionLabel } from "./content";

interface ContentAddDialogProps {
  open: boolean;
  server: Server;
  kind: ContentKind;
  onClose: () => void;
  onInstalled: () => void;
}

/** Hộp thoại tìm kiếm, xem dependency và cài mod/plugin từ Modrinth (spec P3, mục 6.3). */
export function ContentAddDialog({ open, server, kind, onClose, onInstalled }: ContentAddDialogProps) {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<SearchHit | null>(null);
  const [jobId, setJobId] = useState<number | null>(null);
  const [installing, setInstalling] = useState(false);

  function close() {
    setSelected(null);
    setJobId(null);
    setInstalling(false);
    onClose();
  }

  return (
    <Dialog open={open} onClose={close} title={t(`content.${kind}.addTitle`)} size="lg" dismissible={jobId === null && !installing}>
      {jobId !== null ? (
        <InstallJobStep
          jobId={jobId}
          onBack={() => setJobId(null)}
          onClose={close}
          onSucceeded={() => {
            onInstalled();
            close();
          }}
        />
      ) : selected !== null ? (
        <PlanStep
          key={selected.project_id}
          server={server}
          kind={kind}
          project={selected}
          onBack={() => setSelected(null)}
          onPendingChange={setInstalling}
          onStarted={(id) => {
            setInstalling(false);
            setJobId(id);
          }}
        />
      ) : (
        <SearchStep server={server} kind={kind} onSelect={setSelected} onClose={close} />
      )}
    </Dialog>
  );
}

interface SearchStepProps {
  server: Server;
  kind: ContentKind;
  onSelect: (hit: SearchHit) => void;
  onClose: () => void;
}

function SearchStep({ server, kind, onSelect, onClose }: SearchStepProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [submitted, setSubmitted] = useState("");

  useEffect(() => {
    const trimmed = query.trim();
    const timer = window.setTimeout(() => setSubmitted(trimmed), trimmed === "" ? 0 : 300);
    return () => window.clearTimeout(timer);
  }, [query]);

  const searchKind = searchKindFor(kind);
  const search = useInfiniteQuery({
    queryKey: ["content-search", server.id, searchKind, submitted],
    queryFn: ({ pageParam }) =>
      contentApi.search({ source: "modrinth", kind: searchKind, q: submitted, serverId: server.id, page: pageParam }),
    initialPageParam: 1,
    getNextPageParam: (last) => (last.page * last.page_size < last.total ? last.page + 1 : undefined),
  });

  const hits = search.data?.pages.flatMap((page) => page.hits) ?? [];

  return (
    <>
      <DialogBody>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            setSubmitted(query.trim());
          }}
        >
          <SearchInput value={query} onChange={setQuery} label={t(`content.${kind}.searchPlaceholder`)} autoFocus />
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
          <EmptyState icon={Package} title={t("content.addDialog.noResults")} description={t("content.addDialog.noResultsHint")} />
        ) : (
          <Stack gap={2}>
            {hits.map((hit) => (
              <ProjectCard
                key={hit.project_id}
                title={hit.title}
                description={hit.description}
                iconUrl={hit.icon_url}
                downloads={hit.downloads}
                loaders={hitLoaders(hit.categories, server.loader)}
                version={hitVersion(hit.game_versions, server.minecraft)}
                onSelect={() => onSelect(hit)}
              />
            ))}
            {search.hasNextPage && (
              <div>
                <Button variant="secondary" loading={search.isFetchingNextPage} onClick={() => void search.fetchNextPage()}>
                  {t("content.addDialog.loadMore")}
                </Button>
              </div>
            )}
          </Stack>
        )}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" onClick={onClose}>
          {t("ui.close")}
        </Button>
      </DialogFooter>
    </>
  );
}

interface PlanStepProps {
  server: Server;
  kind: ContentKind;
  project: SearchHit;
  onBack: () => void;
  onPendingChange: (pending: boolean) => void;
  onStarted: (jobId: number) => void;
}

function PlanStep({ server, kind, project, onBack, onPendingChange, onStarted }: PlanStepProps) {
  const { t } = useTranslation();
  const [versionId, setVersionId] = useState<string | null>(null);

  const versions = useQuery({
    queryKey: ["content-versions", "modrinth", project.project_id, server.id],
    queryFn: () => contentApi.versions("modrinth", project.project_id, server.id),
  });
  const activeVersionId = versionId ?? versions.data?.[0]?.id ?? null;

  const plan = useQuery({
    queryKey: ["content-plan", server.id, "modrinth", project.project_id, activeVersionId],
    queryFn: () =>
      contentApi.plan(server.id, {
        source: "modrinth",
        project_id: project.project_id,
        version_id: activeVersionId,
      }),
    enabled: activeVersionId !== null,
  });

  const install = useMutation({
    mutationFn: () =>
      contentApi.install(server.id, {
        source: "modrinth",
        project_id: project.project_id,
        version_id: activeVersionId,
      }),
    onMutate: () => onPendingChange(true),
    onError: () => onPendingChange(false),
    onSuccess: (res) => onStarted(res.job_id),
  });

  const noVersions = versions.data !== undefined && versions.data.length === 0;

  return (
    <>
      <DialogBody>
        <ProjectCard
          title={project.title}
          description={project.description}
          iconUrl={project.icon_url}
          downloads={project.downloads}
          loaders={hitLoaders(project.categories, server.loader)}
          version={hitVersion(project.game_versions, server.minecraft)}
        />
        {versions.isPending ? (
          <Spinner label={t("ui.loading")} />
        ) : versions.isError ? (
          <Notice tone="danger">{errorMessage(t, versions.error)}</Notice>
        ) : noVersions ? (
          <Notice tone="warning">{t("errors.CONTENT_INCOMPATIBLE")}</Notice>
        ) : (
          <Field label={t("content.addDialog.version")}>
            <Select value={activeVersionId ?? ""} disabled={install.isPending} onChange={(e) => setVersionId(e.target.value)}>
              {versions.data.map((ver) => (
                <option key={ver.id} value={ver.id}>
                  {versionOptionLabel(ver)}
                </option>
              ))}
            </Select>
          </Field>
        )}
        {activeVersionId !== null && plan.isPending && <Spinner label={t("content.addDialog.planning")} />}
        {plan.isError && <Notice tone="danger">{errorMessage(t, plan.error)}</Notice>}
        {plan.data && <PlanDetails plan={plan.data} kind={kind} />}
        {install.isError && <Notice tone="danger">{errorMessage(t, install.error)}</Notice>}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" icon={ArrowLeft} disabled={install.isPending} onClick={onBack}>
          {t("content.addDialog.back")}
        </Button>
        <Button
          variant="primary"
          disabled={!plan.data || plan.isError || install.isPending}
          loading={install.isPending}
          onClick={() => install.mutate()}
        >
          {t("content.addDialog.install")}
        </Button>
      </DialogFooter>
    </>
  );
}

function PlanDetails({ plan, kind }: { plan: ContentPlan; kind: ContentKind }) {
  const { t, i18n } = useTranslation();
  const mainItems = plan.items.filter((item) => !item.is_dependency);
  const depItems = plan.items.filter((item) => item.is_dependency);
  const hasClientOnly = plan.items.some((item) => item.client_only);

  const renderItem = (item: PlannedContent) => (
    <Stack direction="row" gap={2} align="center" wrap key={item.project_id}>
      <strong>{item.project_name}</strong>
      <Badge>{item.version_number}</Badge>
      <span>{formatBytes(item.file_size, i18n.language)}</span>
      {item.client_only && <Badge tone="warning">{t("content.clientOnly")}</Badge>}
      {item.replaces_existing && <Badge tone="info">{t("content.addDialog.replacesExisting")}</Badge>}
    </Stack>
  );

  return (
    <Stack gap={3}>
      <Card title={t("content.addDialog.planTitle")}>
        <Facts
          items={[
            { label: t(`content.${kind}.mainItem`), value: <Stack gap={2}>{mainItems.map(renderItem)}</Stack> },
            ...(depItems.length > 0
              ? [{ label: t("content.addDialog.dependencies", { count: depItems.length }), value: <Stack gap={2}>{depItems.map(renderItem)}</Stack> }]
              : []),
          ]}
        />
      </Card>
      {hasClientOnly && <Notice tone="info">{t("content.addDialog.clientOnlyHint")}</Notice>}
    </Stack>
  );
}

interface InstallJobStepProps {
  jobId: number;
  onBack: () => void;
  onClose: () => void;
  onSucceeded: () => void;
}

function InstallJobStep({ jobId, onBack, onClose, onSucceeded }: InstallJobStepProps) {
  const { t, i18n } = useTranslation();
  const job = useJob(jobId);
  const cancel = useMutation({ mutationFn: () => api.cancelJob(jobId) });
  const doneRef = useRef(false);
  const state = job.data?.state;

  useEffect(() => {
    if (state === "succeeded" && !doneRef.current) {
      doneRef.current = true;
      onSucceeded();
    }
  }, [state, onSucceeded]);

  if (state === "failed" || state === "cancelled" || job.isError) {
    const error = job.data?.error;
    const notice = job.isError ? (
      <Notice tone="danger">{errorMessage(t, job.error)}</Notice>
    ) : state === "failed" ? (
      <Notice tone="danger" title={t("content.addDialog.failedTitle")}>
        {errorMessage(t, error ? new ApiError(error.code, error.params, 0) : undefined)}
      </Notice>
    ) : (
      <Notice>{t("content.addDialog.cancelledTitle")}</Notice>
    );
    return (
      <>
        <DialogBody>{notice}</DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {t("ui.close")}
          </Button>
          <Button variant={state === "failed" ? "primary" : "secondary"} icon={ArrowLeft} onClick={onBack}>
            {t("content.addDialog.back")}
          </Button>
        </DialogFooter>
      </>
    );
  }

  const progress = job.data?.progress;
  return (
    <>
      <DialogBody>
        <ProgressBar
          label={progress ? jobStepLabel(t, progress, i18n.language) : t("jobs.starting")}
          value={progress ? jobPercent(progress) : undefined}
        />
        {cancel.isError && <Notice tone="danger">{errorMessage(t, cancel.error)}</Notice>}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" loading={cancel.isPending} onClick={() => cancel.mutate()}>
          {t("ui.cancel")}
        </Button>
      </DialogFooter>
    </>
  );
}
