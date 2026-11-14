import { useState } from "react";
import { useTranslation } from "react-i18next";

import { ApiError } from "../../../api/client";
import { contentApi } from "../../../api/content";
import { FileDrop, Notice, ProgressBar } from "../../../ui";
import { contentUploadError, isJarFile } from "./content";

interface ContentUploadProps {
  serverId: string;
  locked: boolean;
  onDone: () => void;
}

/** Vùng kéo thả / chọn file `.jar` để tải lên thư mục mod hoặc plugin (spec P3, mục 6.3 và 8). */
export function ContentUpload({ serverId, locked, onDone }: ContentUploadProps) {
  const { t } = useTranslation();
  const [uploading, setUploading] = useState<string[] | null>(null);
  const [uploadedCount, setUploadedCount] = useState(0);
  const [error, setError] = useState<unknown>(null);

  async function start(files: File[]) {
    setError(null);
    setUploadedCount(0);
    setUploading(files.map((file) => file.name));
    let done = 0;
    try {
      for (const file of files) {
        if (!isJarFile(file)) throw new ApiError("INVALID_INPUT", {}, 400);
        await contentApi.upload(serverId, file);
        done += 1;
      }
      setUploadedCount(done);
    } catch (err) {
      setError(err);
    } finally {
      setUploading(null);
      if (done > 0) onDone();
    }
  }

  return (
    <FileDrop
      label={t("content.upload")}
      hint={t("content.uploadHint")}
      accept=".jar"
      disabled={locked || uploading !== null}
      onFiles={(files) => void start(files)}
    >
      {uploading && <ProgressBar label={t("content.uploading", { names: uploading.join(", ") })} />}
      {uploadedCount > 0 && <Notice tone="success">{t("content.uploaded", { count: uploadedCount })}</Notice>}
      {error !== null && <Notice tone="danger">{contentUploadError(t, error)}</Notice>}
    </FileDrop>
  );
}
