import { useState } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "../../../api/client";
import { uploadFiles } from "../../../api/files";
import { FileDrop, Notice, ProgressBar } from "../../../ui";

interface Upload {
  names: string[];
  percent: number;
}

/** Kéo thả hoặc chọn file để tải lên thư mục đang mở; hiện tiến độ của lượt tải. */
export function UploadZone({ serverId, dir, onDone }: { serverId: string; dir: string; onDone: () => void }) {
  const { t } = useTranslation();
  const [upload, setUpload] = useState<Upload | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [saved, setSaved] = useState<string[]>([]);

  async function start(files: File[]) {
    setError(null);
    setSaved([]);
    setUpload({ names: files.map((f) => f.name), percent: 0 });
    try {
      const names = await uploadFiles(serverId, dir, files, (sent, total) =>
        setUpload((u) => (u ? { ...u, percent: total > 0 ? (sent / total) * 100 : 0 } : u)),
      );
      setSaved(names);
      onDone();
    } catch (e) {
      setError(e);
    } finally {
      setUpload(null);
    }
  }

  return (
    <FileDrop label={t("files.upload")} hint={t("files.uploadHint")} disabled={upload !== null} onFiles={(files) => void start(files)}>
      {upload && <ProgressBar label={t("files.uploading", { names: upload.names.join(", ") })} value={upload.percent} />}
      {saved.length > 0 && <Notice tone="success">{t("files.uploaded", { count: saved.length })}</Notice>}
      {error !== null && <Notice tone="danger">{errorMessage(t, error)}</Notice>}
    </FileDrop>
  );
}
