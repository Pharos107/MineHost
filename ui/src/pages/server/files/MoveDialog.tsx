import { useQuery } from "@tanstack/react-query";
import { ArrowUp, Folder, FolderInput } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "../../../api/client";
import { filesApi, joinPath, parentOf, pathSteps } from "../../../api/files";
import { Breadcrumb, Button, Dialog, DialogBody, DialogFooter, EmptyState, Notice, Spinner, Stack } from "../../../ui";

interface MoveDialogProps {
  serverId: string;
  /** Thư mục đang mở; hộp thoại bắt đầu ở đây. */
  from: string;
  /** Đường dẫn đầy đủ của các mục sẽ chuyển. */
  paths: string[];
  loading: boolean;
  error?: string;
  onCancel: () => void;
  onConfirm: (dest: string) => void;
}

/** Chuyển được vào `dest` khi đó không phải chính mục đang chuyển hay thư mục con của nó. */
export function canMoveInto(dest: string, paths: string[]): boolean {
  return paths.every((path) => dest !== path && !dest.startsWith(`${path}/`));
}

/**
 * Chọn thư mục đích để di chuyển (spec trang Tệp, mục 2): bấm thư mục con để vào, breadcrumb hoặc
 * "Lên trên" để nhảy ra, "Chuyển vào đây" để chọn thư mục đang xem.
 */
export function MoveDialog({ serverId, from, paths, loading, error, onCancel, onConfirm }: MoveDialogProps) {
  const { t } = useTranslation();
  const [dir, setDir] = useState(from);
  const list = useQuery({ queryKey: ["files", serverId, dir], queryFn: () => filesApi.list(serverId, dir) });
  const folders = (list.data ?? []).filter((entry) => entry.kind === "dir" && canMoveInto(joinPath(dir, entry.name), paths));
  const allowed = dir !== from && canMoveInto(dir, paths);

  return (
    <Dialog open onClose={onCancel} title={t("files.move.title", { count: paths.length })}>
      <DialogBody>
        <Stack direction="row" gap={2} align="center" wrap>
          <Button size="sm" variant="ghost" icon={ArrowUp} disabled={dir === ""} onClick={() => setDir(parentOf(dir))}>
            {t("files.move.up")}
          </Button>
          <Breadcrumb
            label={t("files.path")}
            items={[
              { label: t("files.root"), onClick: () => setDir("") },
              ...pathSteps(dir).map((step) => ({ label: step.name, onClick: () => setDir(step.path) })),
            ]}
          />
        </Stack>
        {list.isPending && <Spinner label={t("ui.loading")} />}
        {list.isError && <Notice tone="danger">{errorMessage(t, list.error)}</Notice>}
        {list.isSuccess && folders.length === 0 && <EmptyState icon={Folder} title={t("files.move.noFolders")} />}
        <Stack gap={1}>
          {folders.map((entry) => (
            <Button key={entry.name} variant="ghost" icon={Folder} fullWidth align="start" onClick={() => setDir(joinPath(dir, entry.name))}>
              {entry.name}
            </Button>
          ))}
        </Stack>
        {error && <Notice tone="danger">{error}</Notice>}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" onClick={onCancel}>
          {t("ui.cancel")}
        </Button>
        <Button variant="primary" icon={FolderInput} disabled={!allowed} loading={loading} onClick={() => onConfirm(dir)}>
          {t("files.move.here")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
