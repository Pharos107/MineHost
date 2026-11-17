import { useMutation } from "@tanstack/react-query";
import { File, Folder, Search } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";

import { errorMessage } from "../../../api/client";
import { EDITABLE_MAX_BYTES, filesApi, parentOf, type FoundEntry } from "../../../api/files";
import { formatBytes } from "../../../format";
import { Button, Dialog, DialogBody, DialogFooter, EmptyState, Field, Notice, Stack, TextInput } from "../../../ui";

/** Thư mục: mở thư mục. File văn bản nhỏ: mở trình sửa. File khác: mở thư mục chứa nó. */
export function foundLink(serverId: string, entry: FoundEntry): string {
  const base = `/servers/${serverId}/files`;
  if (entry.kind === "dir") return `${base}?path=${encodeURIComponent(entry.path)}`;
  if ((entry.size ?? 0) <= EDITABLE_MAX_BYTES) return `${base}/edit?path=${encodeURIComponent(entry.path)}`;
  return `${base}?path=${encodeURIComponent(parentOf(entry.path))}`;
}

/** Tìm file theo tên trong thư mục đang mở và mọi thư mục con (spec trang Tệp, mục 2). */
export function SearchDialog({ serverId, dir, onClose }: { serverId: string; dir: string; onClose: () => void }) {
  const { t, i18n } = useTranslation();
  const [query, setQuery] = useState("");
  const search = useMutation({ mutationFn: (q: string) => filesApi.search(serverId, dir, q) });
  const result = search.data;

  return (
    <Dialog open onClose={onClose} title={t("files.search.title")} size="lg">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (query.trim() !== "") search.mutate(query.trim());
        }}
      >
        <DialogBody>
          <Stack direction="row" gap={2} align="end">
            <Field label={t("files.search.label", { dir: dir === "" ? t("files.root") : dir })}>
              <TextInput value={query} autoFocus data-autofocus autoComplete="off" spellCheck={false} onChange={(e) => setQuery(e.target.value)} />
            </Field>
            <Button type="submit" icon={Search} loading={search.isPending} disabled={query.trim() === ""}>
              {t("files.search.submit")}
            </Button>
          </Stack>
          {search.isError && <Notice tone="danger">{errorMessage(t, search.error)}</Notice>}
          {result && result.items.length === 0 && <EmptyState icon={Search} title={t("files.search.none")} />}
          {result && result.items.length > 0 && (
            <Stack gap={2}>
              {result.items.map((entry) => (
                <Link key={entry.path} to={foundLink(serverId, entry)} onClick={onClose}>
                  <Stack direction="row" gap={2} align="center">
                    {entry.kind === "dir" ? <Folder size={16} aria-hidden /> : <File size={16} aria-hidden />}
                    <code>{entry.path}</code>
                    {entry.size != null && <span>{formatBytes(entry.size, i18n.language)}</span>}
                  </Stack>
                </Link>
              ))}
            </Stack>
          )}
          {result?.truncated && <Notice tone="info">{t("files.search.truncated", { count: result.items.length })}</Notice>}
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {t("ui.close")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
