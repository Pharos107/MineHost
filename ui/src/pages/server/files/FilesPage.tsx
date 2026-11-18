import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Archive, Copy, Download, File, FilePlus, Folder, FolderInput, FolderOpen, FolderPlus, FolderUp, HardDrive, Link2, PackageOpen, Pencil, SlidersHorizontal, Trash2, Upload } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Link, useNavigate, useSearchParams } from "react-router";

import { errorMessage } from "../../../api/client";
import { filesApi, joinPath, parentOf, pathSteps, type FileEntry } from "../../../api/files";
import { formatBytes } from "../../../format";
import { Breadcrumb, Button, DataTable, EmptyState, FileDrop, Menu, Notice, Stack, toast, type Column, type DroppedFile, type RowAction } from "../../../ui";
import { useServer } from "../ServerLayout";
import { FileDialogs, type DialogOutcome, type FileDialog } from "./FileDialogs";
import { trashedToast, uploadWithToast, watchJobInToast } from "./fileToasts";
import { useDeepSearch } from "./useDeepSearch";

const rowKey = (e: FileEntry) => `${e.kind}:${e.name}`;
const isArchive = (e: FileEntry) => e.kind === "file" && /\.(zip|tar\.gz|tgz|tar)$/i.test(e.name);
const isImage = (name: string) => /\.(png|jpe?g|gif|webp)$/i.test(name);
const isNbt = (name: string) => /\.(dat|dat_old|nbt)$/i.test(name);
/** File nén, jar, nhị phân thuần không mở được ở trình sửa hay xem trước. */
const isHardBinary = (name: string) => /\.(zip|jar|gz|tgz|tar|7z|rar|mca|exe|dll|so|bin|class)$/i.test(name);

type DirSizeMap = Record<string, { bytes: number; truncated: boolean }>;

/** Trang Tệp (spec P2 mục 6.6, spec trang Tệp, spec nâng cấp trang Tệp). */
export function FilesPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const server = useServer();
  const [params] = useSearchParams();
  const dir = params.get("path") ?? "";
  const queryClient = useQueryClient();
  const list = useQuery({ queryKey: ["files", server.id, dir], queryFn: () => filesApi.list(server.id, dir) });
  const [dialog, setDialog] = useState<FileDialog | null>(null);
  const [dirSizes, setDirSizes] = useState<DirSizeMap>({});
  const fileInput = useRef<HTMLInputElement>(null);
  const dirInput = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (dirInput.current) dirInput.current.webkitdirectory = true;
  }, []);
  const refresh = () => {
    setDirSizes({});
    void queryClient.invalidateQueries({ queryKey: ["files", server.id] });
  };
  // Chọn gắn với thư mục: sang thư mục khác thì coi như bỏ chọn.
  const [picked, setPicked] = useState<{ dir: string; keys: Set<string> }>({ dir, keys: new Set() });
  const selected = picked.dir === dir ? picked.keys : new Set<string>();
  const select = (keys: Set<string>) => setPicked({ dir, keys });
  const search = useDeepSearch(server.id, dir);
  const baseRows = list.data ?? [];
  const rows = search.deep && search.query.trim() !== "" ? (search.rows ?? baseRows) : baseRows;
  const selectedPaths = rows.filter((e) => selected.has(rowKey(e))).map((e) => joinPath(dir, e.name));
  const columns = useColumns(server.id, dir, dirSizes);
  const pathOf = (e: FileEntry) => joinPath(dir, e.name);

  const onDone = (result: DialogOutcome) => {
    setDialog(null);
    select(new Set());
    refresh();
    if (result.trashed && result.trashed.length > 0) trashedToast(t, result.trashed);
    if (result.jobId !== undefined) watchJobInToast(t, result.jobId, refresh);
  };
  const upload = (files: File[]) => void uploadWithToast(t, server.id, dir, files, refresh);
  const uploadDropped = (items: DroppedFile[]) => void uploadWithToast(t, server.id, dir, items, refresh);

  async function calcSize(entry: FileEntry) {
    const full = pathOf(entry);
    try {
      const size = await filesApi.size(server.id, full);
      setDirSizes((prev) => ({ ...prev, [full]: { bytes: size.bytes, truncated: size.truncated } }));
    } catch (e) {
      toast.show({ tone: "danger", title: errorMessage(t, e) });
    }
  }

  const actions: RowAction<FileEntry>[] = [
    {
      label: t("files.download"),
      icon: Download,
      onSelect: (e) => window.location.assign(e.kind === "file" ? filesApi.downloadUrl(server.id, pathOf(e)) : filesApi.archiveUrl(server.id, [pathOf(e)])),
    },
    {
      label: t("files.openFolder"),
      icon: FolderOpen,
      onSelect: (e) => void navigate(`?path=${encodeURIComponent(joinPath(dir, parentOf(e.name)))}`),
      hidden: (e) => !e.name.includes("/"),
    },
    { label: t("files.rename"), icon: Pencil, onSelect: (e) => setDialog({ kind: "rename", entry: e }) },
    { label: t("files.copy.action"), icon: Copy, onSelect: (e) => setDialog({ kind: "copy", entry: e }) },
    { label: t("files.move.action"), icon: FolderInput, onSelect: (e) => setDialog({ kind: "move", paths: [pathOf(e)] }) },
    { label: t("files.sizeCalc.action"), icon: HardDrive, onSelect: (e) => void calcSize(e), hidden: (e) => e.kind !== "dir" },
    { label: t("files.compress.action"), icon: Archive, onSelect: (e) => setDialog({ kind: "compress", paths: [pathOf(e)] }) },
    { label: t("files.extract.action"), icon: PackageOpen, onSelect: (e) => setDialog({ kind: "extract", path: pathOf(e) }), hidden: (e) => !isArchive(e) },
    { label: t("files.delete"), icon: Trash2, tone: "danger", onSelect: (e) => setDialog({ kind: "delete", paths: [pathOf(e)] }) },
  ];

  const toolbar = (
    <>
      <Menu
        size="sm"
        icon={Upload}
        triggerLabel={t("files.uploadMenu")}
        label={t("files.uploadMenu")}
        items={[
          { label: t("files.upload"), icon: Upload, onSelect: () => fileInput.current?.click() },
          { label: t("files.uploadFolder"), icon: FolderUp, onSelect: () => dirInput.current?.click() },
          { label: t("files.fetch.action"), icon: Link2, onSelect: () => setDialog({ kind: "fetch" }) },
        ]}
      />
      <Button size="sm" icon={FilePlus} onClick={() => setDialog({ kind: "file" })}>
        {t("files.newFile")}
      </Button>
      <Button size="sm" icon={FolderPlus} onClick={() => setDialog({ kind: "dir" })}>
        {t("files.newDir")}
      </Button>
      <Button size="sm" icon={Trash2} onClick={() => setDialog({ kind: "trash" })}>
        {t("files.trash.action")}
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
      {/* Kéo file hoặc thư mục thả vào bảng là tải lên đúng thư mục đang mở. */}
      <FileDrop
        variant="overlay"
        label={t("files.upload")}
        hint={t("files.dropHint", { dir: dir === "" ? t("files.root") : dir })}
        onFiles={upload}
        onFilesWithPaths={uploadDropped}
        onEmptyDrop={() => toast.show({ tone: "info", title: t("files.emptyFolderDrop") })}
      >
        {list.isError || search.error ? (
          <Notice tone="danger">{errorMessage(t, list.error ?? search.error)}</Notice>
        ) : (
          <DataTable
            label={t("files.table")}
            columns={columns}
            rows={rows}
            rowKey={rowKey}
            rowLabel={(e) => e.name}
            actions={actions}
            inlineActions={1}
            toolbar={toolbar}
            searchText={(e) => e.name}
            loading={list.isPending}
            searching={search.searching}
            searchValue={search.query}
            searchClientFilter={!search.deep}
            searchLabel={search.deep ? t("files.search.deepLabel") : undefined}
            onSearchChange={search.setQuery}
            searchOptions={
              <Menu
                size="sm"
                icon={SlidersHorizontal}
                active={search.deep}
                label={t("files.search.options")}
                items={[{ label: t("files.search.subfolders"), checked: search.deep, onSelect: search.toggle }]}
              />
            }
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
      </FileDrop>
      {search.truncated && <Notice tone="info">{t("files.search.truncated", { count: rows.length })}</Notice>}
      <input
        ref={fileInput}
        type="file"
        multiple
        hidden
        aria-label={t("files.upload")}
        onChange={(e) => {
          const pickedFiles = e.target.files ? Array.from(e.target.files) : [];
          e.target.value = "";
          if (pickedFiles.length > 0) upload(pickedFiles);
        }}
      />
      <input
        ref={dirInput}
        type="file"
        hidden
        aria-label={t("files.uploadFolder")}
        onChange={(e) => {
          const pickedFiles = e.target.files ? Array.from(e.target.files) : [];
          e.target.value = "";
          if (pickedFiles.length > 0) upload(pickedFiles);
          else toast.show({ tone: "info", title: t("files.emptyFolderDrop") });
        }}
      />
      {dialog && <FileDialogs serverId={server.id} dir={dir} dialog={dialog} onClose={() => setDialog(null)} onDone={onDone} onRefresh={refresh} />}
    </Stack>
  );
}

function useColumns(serverId: string, dir: string, dirSizes: DirSizeMap): Column<FileEntry>[] {
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
      cell: (e) => {
        if (e.kind === "file") return e.size != null ? formatBytes(e.size, i18n.language) : "—";
        const info = dirSizes[joinPath(dir, e.name)];
        if (!info) return "—";
        const formatted = formatBytes(info.bytes, i18n.language);
        return info.truncated ? t("files.sizeCalc.truncated", { size: formatted }) : formatted;
      },
      sortValue: (e) => (e.kind === "file" ? e.size : dirSizes[joinPath(dir, e.name)]?.bytes) ?? -1,
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

/** Thư mục mở thư mục; ảnh, NBT và file văn bản (kể cả log lớn xem cuối) mở trang xem/sửa; nén/jar chỉ tải xuống. */
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
  if (isImage(entry.name) || isNbt(entry.name) || !isHardBinary(entry.name)) {
    return <Link to={`/servers/${serverId}/files/edit?path=${encodeURIComponent(path)}`}>{label}</Link>;
  }
  return label;
}
