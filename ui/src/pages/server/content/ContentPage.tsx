import type { TFunction } from "i18next";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, Package, Plus, RotateCw, Trash2 } from "lucide-react";
import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router";

import { errorMessage } from "../../../api/client";
import { contentApi, contentKey, type InstalledContent } from "../../../api/content";
import { Avatar, Badge, Button, ConfirmDialog, DataTable, EmptyState, Notice, Stack, Switch, type Column, type RowAction } from "../../../ui";
import { useServer } from "../ServerLayout";
import { contentSourceKind, isContentLocked, modrinthProjectUrl } from "./content";
import { ContentAddDialog } from "./ContentAddDialog";
import { ContentUpload } from "./ContentUpload";

/** Trang Mod hoặc Plugin (spec P3, mục 6.3): liệt kê, lọc, thêm, tải lên, bật/tắt và gỡ vào thùng rác. */
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
  const refresh = useCallback(() => void queryClient.invalidateQueries({ queryKey: contentKey(server.id) }), [queryClient, server.id]);

  const [removing, setRemoving] = useState<InstalledContent | null>(null);
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

  if (kind === null) {
    return <EmptyState icon={Package} title={t("content.unsupported")} />;
  }

  const locked = isContentLocked(server);
  const addOpen = !locked && params.get("add") === "1";
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
  const busyFile = toggle.isPending ? toggle.variables.file : null;
  const columns = buildColumns(t, locked, busyFile, (item, enabled) => toggle.mutate({ file: item.file_name, enabled }));
  const actions: RowAction<InstalledContent>[] = [
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
      onSelect: setRemoving,
    },
  ];

  return (
    <Stack gap={4}>
      {locked && <Notice tone="info">{t(`content.${kind}.runningHint`)}</Notice>}
      <Stack direction="row" gap={2} wrap>
        <Button variant="primary" icon={Plus} disabled={locked} onClick={() => setAddOpen(true)}>
          {t("content.add")}
        </Button>
      </Stack>
      <ContentUpload serverId={server.id} locked={locked} onDone={refresh} />
      {toggle.isError && <Notice tone="danger">{errorMessage(t, toggle.error)}</Notice>}
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
          rows={list.data?.files ?? []}
          rowKey={(item) => item.file_name}
          rowLabel={(item) => item.name}
          searchText={(item) => `${item.name} ${item.file_name}`}
          actions={actions}
          inlineActions={0}
          loading={list.isPending}
          empty={<EmptyState icon={Package} title={t(`content.${kind}.empty`)} description={t(`content.${kind}.emptyHint`)} />}
        />
      )}
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
      <ContentAddDialog
        open={addOpen}
        server={server}
        kind={kind}
        onClose={() => setAddOpen(false)}
        onInstalled={refresh}
      />
    </Stack>
  );
}

function buildColumns(
  t: TFunction,
  locked: boolean,
  busyFile: string | null,
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
          disabled={locked || busyFile === item.file_name}
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
