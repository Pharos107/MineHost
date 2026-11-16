import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Check, ListChecks, Plus } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import { errorMessage, type Server } from "../../../api/client";
import { contentApi, contentKey, type ContentKind } from "../../../api/content";
import { SERVERS_KEY } from "../../../api/live";
import { Badge, Button, Dialog, DialogBody, DialogFooter, EmptyState, Notice, Spinner, Stack } from "../../../ui";
import { CatalogBrowser } from "../../catalog/CatalogBrowser";
import type { CatalogActionContext } from "../../catalog/CatalogDetail";
import { MAX_QUEUE, queueItems, queueStorageKey, useContentQueue, type ContentQueue } from "../../catalog/queue";
import { useServer } from "../ServerLayout";
import { isServerRunning, versionOptionLabel } from "./content";
import { InstallJobStep, PlanDetails } from "./ContentInstall";

/** Trang thêm mod hoặc plugin (`/servers/:id/content/add`, spec trình duyệt mod mục 4). */
export function ContentAddPage() {
  const { t } = useTranslation();
  const server = useServer();
  const navigate = useNavigate();
  const queue = useContentQueue(queueStorageKey(server.id));
  const [reviewing, setReviewing] = useState(false);
  const kind = server.content_kind ?? null;
  const back = `/servers/${server.id}/content`;

  if (kind === null) {
    return <EmptyState icon={ListChecks} title={t("content.unsupported")} />;
  }

  const renderAction = ({ hit, version }: CatalogActionContext) => {
    if (version === null) return null;
    if (queue.has(hit.source, hit.project_id)) {
      return (
        <Stack direction="row" gap={2} align="center" wrap>
          <Badge tone="success">{t("catalog.queue.added")}</Badge>
          <Button size="sm" variant="ghost" onClick={() => queue.remove(hit.source, hit.project_id)}>
            {t("catalog.queue.undo")}
          </Button>
        </Stack>
      );
    }
    return (
      <div>
        <Button
          variant="primary"
          icon={Plus}
          disabled={queue.items.length >= MAX_QUEUE}
          onClick={() =>
            queue.add({
              source: hit.source,
              project_id: hit.project_id,
              version_id: version.id,
              title: hit.title,
              version_label: versionOptionLabel(version),
              icon_url: hit.icon_url,
            })
          }
        >
          {t("catalog.queue.add")}
        </Button>
      </div>
    );
  };

  return (
    <Stack gap={4}>
      <Stack direction="row" gap={3} align="center" wrap>
        <Button variant="ghost" icon={ArrowLeft} onClick={() => void navigate(back)}>
          {t(`content.${kind}.backToList`)}
        </Button>
        <strong>{t(`content.${kind}.addTitle`)}</strong>
      </Stack>
      {queue.items.length > 0 && (
        <Notice
          tone="info"
          action={
            <Button size="sm" variant="primary" icon={Check} onClick={() => setReviewing(true)}>
              {t("catalog.queue.review")}
            </Button>
          }
        >
          {t("catalog.queue.summary", { count: queue.items.length, names: queue.items.map((q) => q.title).join(", ") })}
        </Notice>
      )}
      <CatalogBrowser
        kind={kind === "plugins" ? "plugin" : "mod"}
        scope={{ serverId: server.id }}
        loader={server.loader}
        minecraft={server.minecraft}
        renderAction={renderAction}
      />
      <QueueDialog
        open={reviewing}
        server={server}
        kind={kind}
        queue={queue}
        onClose={() => setReviewing(false)}
        onInstalled={() => {
          queue.clear();
          setReviewing(false);
          void navigate(back);
        }}
      />
    </Stack>
  );
}

interface QueueDialogProps {
  open: boolean;
  server: Server;
  kind: ContentKind;
  queue: ContentQueue;
  onClose: () => void;
  onInstalled: () => void;
}

function QueueDialog({ open, server, kind, queue, onClose, onInstalled }: QueueDialogProps) {
  const { t } = useTranslation();
  const [jobId, setJobId] = useState<number | null>(null);
  const close = () => {
    setJobId(null);
    onClose();
  };
  return (
    <Dialog open={open} onClose={close} title={t("catalog.queue.title")} size="lg" dismissible={jobId === null}>
      {jobId !== null ? (
        <InstallJobStep jobId={jobId} onBack={() => setJobId(null)} onClose={close} onSucceeded={onInstalled} />
      ) : (
        <QueuePlan server={server} kind={kind} queue={queue} onClose={close} onStarted={setJobId} />
      )}
    </Dialog>
  );
}

interface QueuePlanProps {
  server: Server;
  kind: ContentKind;
  queue: ContentQueue;
  onClose: () => void;
  onStarted: (jobId: number) => void;
}

function QueuePlan({ server, kind, queue, onClose, onStarted }: QueuePlanProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const items = queueItems(queue.items);
  const plan = useQuery({
    queryKey: ["content-plan", server.id, items],
    queryFn: () => contentApi.plan(server.id, items),
    enabled: items.length > 0,
  });
  const install = useMutation({
    mutationFn: () => contentApi.install(server.id, items),
    onSuccess: (res) => {
      void queryClient.invalidateQueries({ queryKey: contentKey(server.id) });
      void queryClient.invalidateQueries({ queryKey: [...SERVERS_KEY, server.id] });
      onStarted(res.job_id);
    },
  });
  // Server đang chạy thì không thay được jar đang mở; backend cũng trả SERVER_BUSY.
  const blocked = isServerRunning(server) && plan.data ? plan.data.items.filter((item) => item.replaces_existing) : [];

  return (
    <>
      <DialogBody>
        {items.length === 0 ? (
          <p>{t("catalog.queue.empty")}</p>
        ) : plan.isPending ? (
          <Spinner label={t("content.addDialog.planning")} />
        ) : plan.isError ? (
          <Notice tone="danger">{errorMessage(t, plan.error)}</Notice>
        ) : (
          <PlanDetails plan={plan.data} kind={kind} onRemove={(item) => queue.remove(item.source, item.project_id)} />
        )}
        {blocked.length > 0 && (
          <Notice tone="warning">
            {t("content.addDialog.replaceWhileRunning", { names: blocked.map((item) => item.project_name).join(", ") })}
          </Notice>
        )}
        {isServerRunning(server) && blocked.length === 0 && <Notice tone="info">{t(`content.${kind}.addWhileRunning`)}</Notice>}
        {install.isError && <Notice tone="danger">{errorMessage(t, install.error)}</Notice>}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" disabled={install.isPending} onClick={onClose}>
          {t("ui.close")}
        </Button>
        <Button
          variant="primary"
          disabled={!plan.data || plan.isError || install.isPending || blocked.length > 0 || items.length === 0}
          loading={install.isPending}
          onClick={() => install.mutate()}
        >
          {t("catalog.queue.install", { count: items.length })}
        </Button>
      </DialogFooter>
    </>
  );
}
