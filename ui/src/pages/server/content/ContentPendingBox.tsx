import { useMutation } from "@tanstack/react-query";
import { Play } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type Server } from "../../../api/client";
import { contentApi, type PendingContent } from "../../../api/content";
import { Badge, Button, Card, FileDrop, Notice, ProgressBar, Stack } from "../../../ui";

interface ContentPendingBoxProps {
  server: Server;
  pending: PendingContent[];
  locked: boolean;
  onDone: () => void;
}

/**
 * Hộp hiển thị các mod CurseForge tác giả chặn tải tự động qua API (spec P3, mục 6.4):
 * - Mỗi dòng kèm tên mod, link mở trang tải (`page_url`) và trạng thái Chờ / Đã có.
 * - Một vùng kéo thả chung nhận nhiều file và đối chiếu SHA-1 ở backend.
 */
export function ContentPendingBox({ server, pending, locked, onDone }: ContentPendingBoxProps) {
  const { t } = useTranslation();
  // Giữ tạm mục vừa khớp trong phiên hiện tại để dòng chuyển sang "Đã có" thay vì biến mất đột ngột.
  const [resolved, setResolved] = useState<PendingContent[]>([]);
  const [uploading, setUploading] = useState<string[] | null>(null);
  const [errors, setErrors] = useState<{ fileName: string; message: string }[]>([]);

  const startWithout = useMutation({
    mutationFn: () => api.action(server.id, "start", { ignore_preflight: true }),
    onSuccess: onDone,
  });

  const pendingPaths = new Set(pending.map((p) => p.path));
  const visibleResolved = resolved.filter((r) => !pendingPaths.has(r.path));
  if (pending.length === 0 && visibleResolved.length === 0) return null;

  const canStart = server.state === "stopped" || server.state === "crashed";
  const rows = [
    ...pending.map((item) => ({ item, ready: false })),
    ...visibleResolved.map((item) => ({ item, ready: true })),
  ];

  async function handleFiles(files: File[]) {
    setErrors([]);
    setUploading(files.map((f) => f.name));
    try {
      const res = await contentApi.uploadPending(server.id, files);
      const matched: PendingContent[] = [];
      const nextErrors: { fileName: string; message: string }[] = [];
      for (const item of res.results) {
        if (item.ok && item.installed) {
          const found = pending.find((p) => p.file_name === item.installed?.file_name);
          if (found) matched.push(found);
        } else if (item.error) {
          nextErrors.push({ fileName: item.file_name, message: errorMessage(t, item.error) });
        }
      }
      if (matched.length > 0) {
        setResolved((prev) => {
          const seen = new Set(prev.map((p) => p.path));
          return [...prev, ...matched.filter((m) => !seen.has(m.path))];
        });
        onDone();
      }
      setErrors(nextErrors);
    } catch (err) {
      setErrors([{ fileName: files.map((f) => f.name).join(", "), message: errorMessage(t, err) }]);
    } finally {
      setUploading(null);
    }
  }

  return (
    <Card
      title={t("content.pending.title", { count: rows.length })}
      description={t("content.pending.description")}
      actions={
        pending.length > 0 && canStart ? (
          <Button
            variant="secondary"
            icon={Play}
            disabled={locked || uploading !== null}
            loading={startWithout.isPending}
            onClick={() => startWithout.mutate()}
          >
            {t("content.pending.startWithout")}
          </Button>
        ) : undefined
      }
    >
      <Stack gap={3}>
        <Stack gap={2}>
          {rows.map(({ item, ready }) => (
            <Stack key={item.path} direction="row" gap={2} align="center" justify="between" wrap>
              <Stack direction="row" gap={2} align="center" wrap>
                <strong>{item.name}</strong>
                {item.name !== item.file_name && <Badge>{item.file_name}</Badge>}
                <Badge tone={ready ? "success" : "warning"}>
                  {t(ready ? "content.pending.ready" : "content.pending.waiting")}
                </Badge>
              </Stack>
              {!ready && item.page_url && (
                <a href={item.page_url} target="_blank" rel="noreferrer">
                  {t("content.pending.openPage")}
                </a>
              )}
            </Stack>
          ))}
        </Stack>

        {pending.length > 0 ? (
          <FileDrop
            label={t("content.pending.dropLabel")}
            hint={t("content.pending.dropHint")}
            accept=".jar"
            disabled={locked || uploading !== null}
            onFiles={(files) => void handleFiles(files)}
          >
            {uploading && <ProgressBar label={t("content.pending.uploading", { names: uploading.join(", ") })} />}
            {errors.map((err, idx) => (
              <Notice key={`${err.fileName}-${idx}`} tone="danger">
                {`${err.fileName}: ${err.message}`}
              </Notice>
            ))}
          </FileDrop>
        ) : (
          <Notice tone="success">{t("content.pending.allDone")}</Notice>
        )}

        {startWithout.isError && <Notice tone="danger">{errorMessage(t, startWithout.error)}</Notice>}
      </Stack>
    </Card>
  );
}
