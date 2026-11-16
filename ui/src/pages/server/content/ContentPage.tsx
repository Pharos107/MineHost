import type { TFunction } from "i18next";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowUpCircle, ExternalLink, Package, Plus, RotateCw, Trash2 } from "lucide-react";
import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router";

import { api, errorMessage } from "../../../api/client";
import {
  contentApi,
  contentKey,
  type ContentKind,
  type InstalledContent,
  type UpdateCandidate,
  type UpdateReport,
} from "../../../api/content";
import { SERVERS_KEY } from "../../../api/live";
import {
  Avatar,
  Badge,
  Button,
  ConfirmDialog,
  DataTable,
  EmptyState,
  FileButton,
  FileDrop,
  Notice,
  Stack,
  Switch,
  type Column,
  type RowAction,
} from "../../../ui";
import { useServer } from "../ServerLayout";
import { canAddContent, contentSourceKind, isContentLocked, modrinthProjectUrl, updateFor } from "./content";
import { ContentAddDialog } from "./ContentAddDialog";
import { ContentPendingBox } from "./ContentPendingBox";
import { useContentUpdates } from "./ContentUpdates";
import { useContentUpload } from "./ContentUpload";

/**
 * Trang Mod hoặc Plugin (spec trình duyệt mod, mục 1): thanh công cụ một hàng trên bảng, kéo thả
 * `.jar` thẳng vào bảng. Thêm mod được cả khi server chạy; bật/tắt và gỡ thì phải tắt server.
 */
export function ContentPage() {
  const { t } = useTranslation();
  const server = useServer();
  const kind = server.content_kind ?? null;
  const [params, setParams] = useSearchParams();
  const queryClient = useQueryClient();
  const list = useQuery({
    queryKey: contentKey(server.id),
    queryFn: () => contentApi.list(server.id),
    enabled: kind !== null,
  });
  // Đổi mod thì kết quả kiểm tra trước khi bật cũng đổi: làm mới cả băng cảnh báo, không chờ WebSocket.
  const refresh = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: contentKey(server.id) });
    void queryClient.invalidateQueries({ queryKey: [...SERVERS_KEY, server.id] });
    void queryClient.invalidateQueries({ queryKey: [...SERVERS_KEY, server.id, "preflight"] });
  }, [queryClient, server.id]);

  const [removing, setRemoving] = useState<InstalledContent | null>(null);
  const [report, setReport] = useState<UpdateReport | null>(null);
  const [confirming, setConfirming] = useState<UpdateCandidate[] | null>(null);
  const toggle = useMutation({
    mutationFn: ({ file, enabled }: { file: string; enabled: boolean }) => contentApi.toggle(server.id, file, enabled),
    onSuccess: refresh,
  });
  const remove = useMutation({
    mutationFn: (item: InstalledContent) => contentApi.remove(server.id, item.file_name),
    onSuccess: () => {
      setRemoving(null);
      refresh();
    },
  });
  const upload = useContentUpload(server.id, refresh);
  const updates = useContentUpdates({
    serverId: server.id,
    kind: kind ?? "mods",
    report,
    onReport: setReport,
    confirming,
    onConfirming: setConfirming,
    onUpdated: refresh,
  });

  if (kind === null) {
    return <EmptyState icon={Package} title={t("content.unsupported")} />;
  }

  const locked = isContentLocked(server);
  const canAdd = canAddContent(server);
  const addOpen = canAdd && params.get("add") === "1";
  const setAddOpen = (open: boolean) => {
    setParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        if (open) next.set("add", "1");
        else next.delete("add");
        return next;
      },
      { replace: true },
    );
  };
  const files = list.data?.files ?? [];
  const waiting = files.filter((item) => item.needs_restart).length;
  const busyFile = toggle.isPending ? toggle.variables.file : null;
  const lockHint = locked ? t(`content.${kind}.lockedHint`) : undefined;
  const columns = buildColumns(t, lockHint, busyFile, report, (item, enabled) => toggle.mutate({ file: item.file_name, enabled }));
  const actions = buildActions(t, kind, locked, report, setConfirming, setRemoving);
  const toolbar = (
    <>
      <Button variant="primary" icon={Plus} disabled={!canAdd} onClick={() => setAddOpen(true)}>
        {t(`content.${kind}.add`)}
      </Button>
      <FileButton label={t("content.upload")} accept=".jar" disabled={!canAdd} loading={upload.busy} onFiles={upload.start} />
      {updates.button}
    </>
  );

  return (
    <Stack gap={3}>
      <ContentPendingBox key={server.id} server={server} pending={list.data?.pending ?? []} locked={locked} onDone={refresh} />
      {waiting > 0 && <RestartNotice serverId={server.id} kind={kind} count={waiting} />}
      {upload.status}
      {updates.panel}
      {toggle.isError && <Notice tone="danger">{errorMessage(t, toggle.error)}</Notice>}
      <FileDrop variant="overlay" label={t("content.upload")} hint={t("content.dropHint")} disabled={!canAdd} onFiles={upload.start}>
        {list.isError ? (
          <Notice
            tone="danger"
            action={
              <Button size="sm" icon={RotateCw} onClick={() => void list.refetch()}>
                {t("common.retry")}
              </Button>
            }
          >
            {errorMessage(t, list.error)}
          </Notice>
        ) : (
          <DataTable
            label={t(`content.${kind}.table`)}
            columns={columns}
            rows={files}
            rowKey={(item) => item.file_name}
            rowLabel={(item) => item.name}
            searchText={(item) => `${item.name} ${item.file_name}`}
            toolbar={toolbar}
            actions={actions}
            inlineActions={0}
            loading={list.isPending}
            empty={
              <Stack gap={3}>
                <Stack direction="row" gap={2} wrap>
                  {toolbar}
                </Stack>
                <EmptyState icon={Package} title={t(`content.${kind}.empty`)} description={t(`content.${kind}.emptyHint`)} />
              </Stack>
            }
          />
        )}
      </FileDrop>
      <RemoveDialog
        item={removing}
        loading={remove.isPending}
        error={remove.isError ? remove.error : null}
        onCancel={() => {
          setRemoving(null);
          remove.reset();
        }}
        onConfirm={() => removing && remove.mutate(removing)}
      />
      <ContentAddDialog open={addOpen} server={server} kind={kind} onClose={() => setAddOpen(false)} onInstalled={refresh} />
    </Stack>
  );
}

/** Báo mod thêm lúc server đang chạy, kèm nút khởi động lại để nạp. */
function RestartNotice({ serverId, kind, count }: { serverId: string; kind: ContentKind; count: number }) {
  const { t } = useTranslation();
  const restart = useMutation({ mutationFn: () => api.action(serverId, "restart") });
  return (
    <Notice
      tone="info"
      action={
        <Button size="sm" icon={RotateCw} loading={restart.isPending} onClick={() => restart.mutate()}>
          {t("server.restart")}
        </Button>
      }
    >
      {t(`content.${kind}.restartNotice`, { count })}
      {restart.isError && <> {errorMessage(t, restart.error)}</>}
    </Notice>
  );
}

function buildActions(
  t: TFunction,
  kind: ContentKind,
  locked: boolean,
  report: UpdateReport | null,
  onConfirm: (items: UpdateCandidate[]) => void,
  onRemove: (item: InstalledContent) => void,
): RowAction<InstalledContent>[] {
  return [
    {
      label: t("content.updates.updateOne"),
      icon: ArrowUpCircle,
      hidden: (item) => updateFor(report, item) === undefined,
      onSelect: (item) => {
        const candidate = updateFor(report, item);
        if (candidate) onConfirm([candidate]);
      },
    },
    {
      label: t(`content.${kind}.openPage`),
      icon: ExternalLink,
      hidden: (item) => modrinthProjectUrl(item) === null,
      onSelect: (item) => {
        const url = modrinthProjectUrl(item);
        if (url) window.open(url, "_blank", "noopener,noreferrer");
      },
    },
    {
      label: t("content.remove"),
      icon: Trash2,
      tone: "danger",
      disabled: () => locked,
      onSelect: onRemove,
    },
  ];
}

function buildColumns(
  t: TFunction,
  lockHint: string | undefined,
  busyFile: string | null,
  report: UpdateReport | null,
  onToggle: (item: InstalledContent, enabled: boolean) => void,
): Column<InstalledContent>[] {
  return [
    {
      id: "name",
      header: t("content.name"),
      cell: (item) => (
        <Stack direction="row" gap={2} align="center" wrap>
          <Avatar name={item.name} imageUrl={item.icon_url} icon={Package} size="sm" />
          <span>{item.name}</span>
          {item.client_only && <Badge tone="warning">{t("content.clientOnly")}</Badge>}
          {item.needs_restart && <Badge tone="info">{t("content.needsRestart")}</Badge>}
          {updateFor(report, item) && <Badge tone="info">{t("content.updates.badge")}</Badge>}
        </Stack>
      ),
      sortValue: (item) => item.name.toLowerCase(),
    },
    {
      id: "version",
      header: t("content.version"),
      cell: (item) => item.version ?? "—",
      sortValue: (item) => item.version ?? "",
      width: "sm",
    },
    {
      id: "source",
      header: t("content.source"),
      cell: (item) => t(`content.sources.${contentSourceKind(item)}`),
      sortValue: (item) => t(`content.sources.${contentSourceKind(item)}`),
      width: "sm",
    },
    {
      id: "enabled",
      header: t("content.enabled"),
      cell: (item) => (
        <Switch
          label={t("content.toggleLabel", { name: item.name })}
          hideLabel
          checked={item.enabled}
          disabled={lockHint !== undefined || busyFile === item.file_name}
          title={lockHint}
          onChange={(enabled) => onToggle(item, enabled)}
        />
      ),
      sortValue: (item) => (item.enabled ? 0 : 1),
      align: "center",
      width: "xs",
    },
  ];
}

interface RemoveDialogProps {
  item: InstalledContent | null;
  loading: boolean;
  error: unknown;
  onCancel: () => void;
  onConfirm: () => void;
}

function RemoveDialog({ item, loading, error, onCancel, onConfirm }: RemoveDialogProps) {
  const { t } = useTranslation();
  return (
    <ConfirmDialog
      open={item !== null}
      title={t("content.removeTitle", { name: item?.name ?? "" })}
      confirmLabel={t("content.remove")}
      loading={loading}
      onCancel={onCancel}
      onConfirm={onConfirm}
    >
      <p>{t("content.removeBody", { file: item?.file_name ?? "" })}</p>
      {error !== null && <Notice tone="danger">{errorMessage(t, error)}</Notice>}
    </ConfirmDialog>
  );
}
