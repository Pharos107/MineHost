import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { ApiError, errorMessage } from "../../../api/client";
import { filesApi, joinPath, parentOf, type FileEntry } from "../../../api/files";
import { Button, ChoiceGroup, ConfirmDialog, Dialog, DialogBody, DialogFooter, Notice } from "../../../ui";
import { FetchUrlDialog } from "./FetchUrlDialog";
import { MoveDialog } from "./MoveDialog";
import { NameDialog } from "./NameDialog";
import { TrashDialog } from "./TrashDialog";

export type FileDialog =
  | { kind: "file" }
  | { kind: "dir" }
  | { kind: "rename"; entry: FileEntry }
  | { kind: "copy"; entry: FileEntry }
  | { kind: "move"; paths: string[] }
  | { kind: "compress"; paths: string[] }
  | { kind: "extract"; path: string }
  | { kind: "delete"; paths: string[] }
  | { kind: "fetch" }
  | { kind: "trash" };

/** Kết quả để trang hiện thông báo: đường dẫn trong thùng rác, hoặc Job nén / giải nén vừa chạy. */
export interface DialogOutcome {
  trashed?: string[];
  jobId?: number;
}

interface FileDialogsProps {
  serverId: string;
  dir: string;
  dialog: FileDialog;
  onClose: () => void;
  onDone: (outcome: DialogOutcome) => void;
  onRefresh: () => void;
}

export function baseName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1);
}

/** Lỗi kèm mục gây lỗi (giải nén trùng file trả `params.path`). */
function describe(t: ReturnType<typeof useTranslation>["t"], error: unknown): string {
  const message = errorMessage(t, error);
  const path = error instanceof ApiError ? error.params.path : undefined;
  return typeof path === "string" ? `${message} (${path})` : message;
}

/** Mọi hộp thoại của trang Tệp và lệnh gọi API của chúng (spec trang Tệp, mục 2). */
export function FileDialogs({ serverId, dir, dialog, onClose, onDone, onRefresh }: FileDialogsProps) {
  const { t } = useTranslation();
  const run = useMutation({
    mutationFn: async (work: () => Promise<DialogOutcome>) => work(),
    onSuccess: onDone,
  });
  const error = run.isError ? describe(t, run.error) : undefined;
  const close = () => {
    run.reset();
    onClose();
  };
  const common = { loading: run.isPending, error, onCancel: close };

  switch (dialog.kind) {
    case "file":
    case "dir":
    case "rename":
    case "copy": {
      const entry = dialog.kind === "rename" || dialog.kind === "copy" ? dialog.entry : null;
      const initial =
        dialog.kind === "rename" && entry
          ? baseName(entry.name)
          : dialog.kind === "copy" && entry
            ? defaultCopyName(baseName(entry.name), t)
            : "";
      return (
        <NameDialog
          {...common}
          title={dialog.kind === "copy" && entry ? t("files.copy.title", { name: baseName(entry.name) }) : t(`files.dialog.${dialog.kind}`)}
          label={dialog.kind === "dir" ? t("files.dirName") : dialog.kind === "copy" ? t("files.copy.label") : t("files.fileName")}
          confirmLabel={dialog.kind === "rename" ? t("files.rename") : dialog.kind === "copy" ? t("files.copy.submit") : t("files.create")}
          initial={initial}
          onConfirm={(name) =>
            run.mutate(async () => {
              if (dialog.kind === "file") await filesApi.write(serverId, joinPath(dir, name), "");
              else if (dialog.kind === "dir") await filesApi.mkdir(serverId, joinPath(dir, name));
              else if (dialog.kind === "copy" && entry) {
                const from = joinPath(dir, entry.name);
                await filesApi.copy(serverId, from, joinPath(parentOf(from), name));
              } else if (entry) {
                // Kết quả tìm trong thư mục con có tên dạng `a/b.yml`: đổi tên ngay trong thư mục chứa nó.
                const from = joinPath(dir, entry.name);
                await filesApi.rename(serverId, from, joinPath(parentOf(from), name));
              }
              return {};
            })
          }
        />
      );
    }
    case "fetch":
      return (
        <FetchUrlDialog
          {...common}
          onConfirm={(url, filename) =>
            run.mutate(async () => ({ jobId: (await filesApi.fetch(serverId, dir, url, filename)).job_id }))
          }
        />
      );
    case "trash":
      return <TrashDialog serverId={serverId} onClose={close} onChanged={onRefresh} />;
    case "move":
      return (
        <MoveDialog
          {...common}
          serverId={serverId}
          from={dir}
          paths={dialog.paths}
          onConfirm={(dest) =>
            run.mutate(async () => {
              // Lần lượt, dừng ở mục lỗi đầu tiên: các mục trước đó đã chuyển xong.
              for (const path of dialog.paths) await filesApi.rename(serverId, path, joinPath(dest, baseName(path)));
              return {};
            })
          }
        />
      );
    case "compress":
      return (
        <NameDialog
          {...common}
          title={t("files.compress.title", { count: dialog.paths.length })}
          label={t("files.compress.name")}
          confirmLabel={t("files.compress.submit")}
          initial={dialog.paths.length === 1 ? `${baseName(dialog.paths[0] ?? "")}.zip` : t("files.compress.defaultName")}
          onConfirm={(name) => run.mutate(async () => ({ jobId: (await filesApi.compress(serverId, dialog.paths, joinPath(dir, name))).job_id }))}
        />
      );
    case "extract":
      return <ExtractDialog {...common} dir={dir} path={dialog.path} onConfirm={(dest) => run.mutate(async () => ({ jobId: (await filesApi.extract(serverId, dialog.path, dest)).job_id }))} />;
    case "delete":
      return (
        <ConfirmDialog
          open
          title={dialog.paths.length === 1 ? t("files.deleteTitle", { name: baseName(dialog.paths[0] ?? "") }) : t("files.deleteMany", { count: dialog.paths.length })}
          confirmLabel={t("files.delete")}
          loading={run.isPending}
          onCancel={close}
          onConfirm={() =>
            run.mutate(async () => {
              const trashed: string[] = [];
              for (const path of dialog.paths) trashed.push((await filesApi.remove(serverId, path)).trash_path);
              return { trashed };
            })
          }
        >
          <p>{t("files.deleteBody")}</p>
          {error && <Notice tone="danger">{error}</Notice>}
        </ConfirmDialog>
      );
  }
}

interface ExtractDialogProps {
  dir: string;
  path: string;
  loading: boolean;
  error?: string;
  onCancel: () => void;
  onConfirm: (dest: string) => void;
}

/** Giải nén vào thư mục mới mang tên file zip (mặc định) hoặc vào thư mục đang mở. */
function ExtractDialog({ dir, path, loading, error, onCancel, onConfirm }: ExtractDialogProps) {
  const { t } = useTranslation();
  const [where, setWhere] = useState("new");
  const folder = baseName(path).replace(/\.(zip|tar\.gz|tgz|tar)$/i, "");
  return (
    <Dialog open onClose={onCancel} title={t("files.extract.title", { name: baseName(path) })}>
      <DialogBody>
        <ChoiceGroup
          label={t("files.extract.where")}
          value={where}
          onChange={setWhere}
          options={[
            { value: "new", label: t("files.extract.newFolder", { name: folder }), description: t("files.extract.newFolderHint") },
            { value: "here", label: t("files.extract.here"), description: t("files.extract.hereHint") },
          ]}
        />
        {error && <Notice tone="danger">{error}</Notice>}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" onClick={onCancel}>
          {t("ui.cancel")}
        </Button>
        <Button variant="primary" loading={loading} onClick={() => onConfirm(where === "new" ? joinPath(dir, folder) : dir)}>
          {t("files.extract.submit")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

function defaultCopyName(name: string, t: ReturnType<typeof useTranslation>["t"]): string {
  const dot = name.lastIndexOf(".");
  if (dot > 0) return `${t("files.copy.suffix", { name: name.slice(0, dot) })}${name.slice(dot)}`;
  return t("files.copy.suffix", { name });
}
