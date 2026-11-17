import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Archive, Download, File, FilePlus, Folder, FolderInput, FolderPlus, PackageOpen, Pencil, Search, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link, useSearchParams } from "react-router";

import { errorMessage, type Job } from "../../../api/client";
import { EDITABLE_MAX_BYTES, filesApi, joinPath, pathSteps, type FileEntry } from "../../../api/files";
import { formatBytes } from "../../../format";
import { Breadcrumb, Button, DataTable, EmptyState, Notice, Stack, type Column, type RowAction } from "../../../ui";
import { JobProgress, jobError } from "../JobProgress";
import { useServer } from "../ServerLayout";
import { FileDialogs, type DialogOutcome, type FileDialog } from "./FileDialogs";
import { UploadZone } from "./UploadZone";

const rowKey = (e: FileEntry) => `${e.kind}:${e.name}`;
const isZip = (e: FileEntry) => e.kind === "file" && e.name.toLowerCase().endsWith(".zip");

/** Trang Tệp (spec P2 mục 6.6, spec trang Tệp mục 2): duyệt, chọn nhiều, di chuyển, tìm, nén, giải nén. */
export function FilesPage() {
  const { t } = useTranslation();
  const server = useServer();
  const [params] = useSearchParams();
  const dir = params.get("path") ?? "";
  const queryClient = useQueryClient();
  const list = useQuery({ queryKey: ["files", server.id, dir], queryFn: () => filesApi.list(server.id, dir) });
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["files", server.id] });
  const [dialog, setDialog] = useState<FileDialog | null>(null);
  const [outcome, setOutcome] = useState<DialogOutcome & { failed?: Job }>({});
  // Chọn gắn với thư mục: sang thư mục khác thì coi như bỏ chọn.
  const [picked, setPicked] = useState<{ dir: string; keys: Set<string> }>({ dir, keys: new Set() });
  const selected = picked.dir === dir ? picked.keys : new Set<string>();
  const select = (keys: Set<string>) => setPicked({ dir, keys });
  const selectedPaths = (list.data ?? []).filter((e) => selected.has(rowKey(e))).map((e) => joinPath(dir, e.name));
  const columns = useColumns(server.id, dir);
  const pathOf = (e: FileEntry) => joinPath(dir, e.name);

  const onDone = (result: DialogOutcome) => {
    setDialog(null);
    setOutcome(result);
    select(new Set());
    refresh();
  };
  const onJobFinish = (job: Job) => {
    setOutcome(job.state === "failed" ? { failed: job } : {});
    refresh();
  };

  const actions: RowAction<FileEntry>[] = [
    {
      label: t("files.download"),
      icon: Download,
      onSelect: (e) => window.location.assign(e.kind === "file" ? filesApi.downloadUrl(server.id, pathOf(e)) : filesApi.archiveUrl(server.id, [pathOf(e)])),
    },
    { label: t("files.rename"), icon: Pencil, onSelect: (e) => setDialog({ kind: "rename", entry: e }) },
    { label: t("files.move.action"), icon: FolderInput, onSelect: (e) => setDialog({ kind: "move", paths: [pathOf(e)] }) },
    { label: t("files.compress.action"), icon: Archive, onSelect: (e) => setDialog({ kind: "compress", paths: [pathOf(e)] }) },
    { label: t("files.extract.action"), icon: PackageOpen, onSelect: (e) => setDialog({ kind: "extract", path: pathOf(e) }), hidden: (e) => !isZip(e) },
    { label: t("files.delete"), icon: Trash2, tone: "danger", onSelect: (e) => setDialog({ kind: "delete", paths: [pathOf(e)] }) },
  ];

  const toolbar = (
    <>
      <Button size="sm" icon={FilePlus} onClick={() => setDialog({ kind: "file" })}>
        {t("files.newFile")}
      </Button>
      <Button size="sm" icon={FolderPlus} onClick={() => setDialog({ kind: "dir" })}>
        {t("files.newDir")}
      </Button>
      <Button size="sm" icon={Search} onClick={() => setDialog({ kind: "search" })}>
        {t("files.search.action")}
      </Button>
    </>
  );
  const bulk = (
    <>
      <Button size="sm" icon={Download} onClick={() => window.location.assign(filesApi.archiveUrl(server.id, selectedPaths))}>
        {t("files.download")}
      </Button>
      <Button size="sm" icon={FolderInput} onClick={() => setDialog({ kind: "move", paths: selectedPaths })}>
        {t("files.move.action")}
      </Button>
      <Button size="sm" icon={Archive} onClick={() => setDialog({ kind: "compress", paths: selectedPaths })}>
        {t("files.compress.action")}
      </Button>
      <Button size="sm" icon={Trash2} variant="danger" onClick={() => setDialog({ kind: "delete", paths: selectedPaths })}>
        {t("files.delete")}
      </Button>
    </>
  );

  return (
    <Stack gap={4}>
      <Breadcrumb
        label={t("files.path")}
        items={[{ label: t("files.root"), to: "?path=" }, ...pathSteps(dir).map((s) => ({ label: s.name, to: `?path=${encodeURIComponent(s.path)}` }))]}
      />
      <Outcome outcome={outcome} onJobFinish={onJobFinish} />
      <UploadZone serverId={server.id} dir={dir} onDone={refresh} />
      {list.isError ? (
        <Notice tone="danger">{errorMessage(t, list.error)}</Notice>
      ) : (
        <DataTable
          label={t("files.table")}
          columns={columns}
          rows={list.data ?? []}
          rowKey={rowKey}
          rowLabel={(e) => e.name}
          actions={actions}
          inlineActions={1}
          toolbar={toolbar}
          searchText={(e) => e.name}
          loading={list.isPending}
          selection={{ selected, onChange: select, actions: bulk }}
          empty={
            <Stack gap={3}>
              {/* Thư mục trống thì bảng chỉ hiện màn hình trống, nút tạo vẫn phải còn. */}
              <Stack direction="row" gap={2} wrap>
                {toolbar}
              </Stack>
              <EmptyState icon={Folder} title={t("files.empty")} />
            </Stack>
          }
        />
      )}
      {dialog && <FileDialogs serverId={server.id} dir={dir} dialog={dialog} onClose={() => setDialog(null)} onDone={onDone} />}
    </Stack>
  );
}

/** Thông báo sau thao tác: mục đã vào thùng rác, tiến độ Job nén / giải nén, hoặc lỗi của Job đó. */
function Outcome({ outcome, onJobFinish }: { outcome: DialogOutcome & { failed?: Job }; onJobFinish: (job: Job) => void }) {
  const { t } = useTranslation();
  if (outcome.jobId !== undefined) return <JobProgress key={outcome.jobId} jobId={outcome.jobId} onFinish={onJobFinish} />;
  if (outcome.failed) return <Notice tone="danger">{errorMessage(t, jobError(outcome.failed))}</Notice>;
  if (outcome.trashed && outcome.trashed.length > 0) {
    return (
      <Notice tone="success" title={t("files.trashed")}>
        {outcome.trashed.map((path) => (
          <div key={path}>
            <code>{path}</code>
          </div>
        ))}
      </Notice>
    );
  }
  return null;
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
