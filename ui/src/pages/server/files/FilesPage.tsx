import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Download, File, FilePlus, Folder, FolderPlus, Pencil, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link, useSearchParams } from "react-router";

import { errorMessage } from "../../../api/client";
import { EDITABLE_MAX_BYTES, filesApi, joinPath, pathSteps, type FileEntry } from "../../../api/files";
import { formatBytes } from "../../../format";
import { Breadcrumb, Button, ConfirmDialog, DataTable, EmptyState, Notice, Stack, type Column, type RowAction } from "../../../ui";
import { useServer } from "../ServerLayout";
import { NameDialog } from "./NameDialog";
import { UploadZone } from "./UploadZone";

type Dialog = { kind: "file" } | { kind: "dir" } | { kind: "rename"; entry: FileEntry } | null;

/** Trang Tệp (spec P2, mục 6.6): duyệt, tạo, đổi tên, xóa vào thùng rác, tải lên và tải xuống. */
export function FilesPage() {
  const { t } = useTranslation();
  const server = useServer();
  const [params] = useSearchParams();
  const dir = params.get("path") ?? "";
  const queryClient = useQueryClient();
  const list = useQuery({ queryKey: ["files", server.id, dir], queryFn: () => filesApi.list(server.id, dir) });
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["files", server.id] });
  const [dialog, setDialog] = useState<Dialog>(null);
  const [deleting, setDeleting] = useState<FileEntry | null>(null);
  const [trashed, setTrashed] = useState<string | null>(null);
  const columns = useColumns(server.id, dir);

  const save = useMutation({
    mutationFn: async ({ d, name }: { d: NonNullable<Dialog>; name: string }) => {
      if (d.kind === "file") return filesApi.write(server.id, joinPath(dir, name), "");
      if (d.kind === "dir") return filesApi.mkdir(server.id, joinPath(dir, name));
      return filesApi.rename(server.id, joinPath(dir, d.entry.name), joinPath(dir, name));
    },
    onSuccess: () => {
      setDialog(null);
      refresh();
    },
  });
  const remove = useMutation({
    mutationFn: (entry: FileEntry) => filesApi.remove(server.id, joinPath(dir, entry.name)),
    onSuccess: (result) => {
      setDeleting(null);
      setTrashed(result.trash_path);
      refresh();
    },
  });

  const actions: RowAction<FileEntry>[] = [
    {
      label: t("files.download"),
      icon: Download,
      onSelect: (e) => window.location.assign(filesApi.downloadUrl(server.id, joinPath(dir, e.name))),
      hidden: (e) => e.kind !== "file",
    },
    { label: t("files.rename"), icon: Pencil, onSelect: (e) => setDialog({ kind: "rename", entry: e }) },
    { label: t("files.delete"), icon: Trash2, tone: "danger", onSelect: setDeleting },
  ];

  return (
    <Stack gap={4}>
      <Breadcrumb
        label={t("files.path")}
        items={[{ label: t("files.root"), to: "?path=" }, ...pathSteps(dir).map((s) => ({ label: s.name, to: `?path=${encodeURIComponent(s.path)}` }))]}
      />
      {trashed && (
        <Notice tone="success" title={t("files.trashed")}>
          <code>{trashed}</code>
        </Notice>
      )}
      {/* Nằm ngoài bảng: thư mục trống thì bảng chỉ hiện màn hình trống, nút vẫn phải còn. */}
      <Stack direction="row" gap={2} wrap>
        <Button size="sm" icon={FilePlus} onClick={() => setDialog({ kind: "file" })}>
          {t("files.newFile")}
        </Button>
        <Button size="sm" icon={FolderPlus} onClick={() => setDialog({ kind: "dir" })}>
          {t("files.newDir")}
        </Button>
      </Stack>
      <UploadZone serverId={server.id} dir={dir} onDone={refresh} />
      {list.isError ? (
        <Notice tone="danger">{errorMessage(t, list.error)}</Notice>
      ) : (
        <DataTable
          label={t("files.table")}
          columns={columns}
          rows={list.data ?? []}
          rowKey={(e) => `${e.kind}:${e.name}`}
          rowLabel={(e) => e.name}
          actions={actions}
          inlineActions={1}
          searchText={(e) => e.name}
          loading={list.isPending}
          empty={<EmptyState icon={Folder} title={t("files.empty")} />}
        />
      )}
      {dialog && (
        <NameDialog
          title={t(`files.dialog.${dialog.kind}`)}
          label={dialog.kind === "dir" ? t("files.dirName") : t("files.fileName")}
          confirmLabel={dialog.kind === "rename" ? t("files.rename") : t("files.create")}
          initial={dialog.kind === "rename" ? dialog.entry.name : ""}
          loading={save.isPending}
          error={save.isError ? errorMessage(t, save.error) : undefined}
          onCancel={() => {
            setDialog(null);
            save.reset();
          }}
          onConfirm={(name) => save.mutate({ d: dialog, name })}
        />
      )}
      <ConfirmDialog
        open={deleting !== null}
        title={t("files.deleteTitle", { name: deleting?.name ?? "" })}
        confirmLabel={t("files.delete")}
        loading={remove.isPending}
        onCancel={() => {
          setDeleting(null);
          remove.reset();
        }}
        onConfirm={() => deleting && remove.mutate(deleting)}
      >
        <p>{t("files.deleteBody")}</p>
        {remove.isError && <Notice tone="danger">{errorMessage(t, remove.error)}</Notice>}
      </ConfirmDialog>
    </Stack>
  );
}

function useColumns(serverId: string, dir: string): Column<FileEntry>[] {
  const { t, i18n } = useTranslation();
  const time = new Intl.DateTimeFormat(i18n.language, { dateStyle: "short", timeStyle: "short" });
  return [
    {
      id: "name",
      header: t("files.name"),
      cell: (e) => <EntryName serverId={serverId} dir={dir} entry={e} />,
      sortValue: (e) => `${e.kind === "dir" ? 0 : 1}${e.name.toLowerCase()}`,
    },
    {
      id: "size",
      header: t("files.size"),
      cell: (e) => (e.kind === "file" && e.size != null ? formatBytes(e.size, i18n.language) : "—"),
      sortValue: (e) => e.size ?? -1,
      align: "end",
      width: "sm",
    },
    {
      id: "modified",
      header: t("files.modified"),
      cell: (e) => (e.modified ? time.format(new Date(e.modified)) : "—"),
      sortValue: (e) => e.modified ?? "",
      width: "md",
    },
  ];
}

/** Thư mục thì mở thư mục; file văn bản nhỏ thì mở trình sửa; file lớn chỉ tải xuống (menu). */
function EntryName({ serverId, dir, entry }: { serverId: string; dir: string; entry: FileEntry }) {
  const path = joinPath(dir, entry.name);
  const Icon = entry.kind === "dir" ? Folder : File;
  const label = (
    <Stack direction="row" gap={2} align="center">
      <Icon size={16} aria-hidden />
      <span>{entry.name}</span>
    </Stack>
  );
  if (entry.kind === "dir") return <Link to={`?path=${encodeURIComponent(path)}`}>{label}</Link>;
  if ((entry.size ?? 0) <= EDITABLE_MAX_BYTES) {
    return <Link to={`/servers/${serverId}/files/edit?path=${encodeURIComponent(path)}`}>{label}</Link>;
  }
  return label;
}
