import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { ApiError } from "../../../api/client";
import { contentApi } from "../../../api/content";
import { Notice, ProgressBar } from "../../../ui";
import { contentUploadError, isJarFile } from "./content";

interface ContentUpload {
  /** Tải lần lượt từng file; dừng ở file lỗi đầu tiên. */
  start: (files: File[]) => void;
  busy: boolean;
  /** Tiến độ và kết quả, đặt dưới thanh công cụ. `null` khi không có gì để báo. */
  status: ReactNode;
}

/** Tải file `.jar` lên thư mục mod hoặc plugin (spec trình duyệt mod, mục 1). */
export function useContentUpload(serverId: string, onDone: () => void): ContentUpload {
  const { t } = useTranslation();
  const [uploading, setUploading] = useState<string[] | null>(null);
  const [uploadedCount, setUploadedCount] = useState(0);
  const [error, setError] = useState<unknown>(null);

  async function run(files: File[]) {
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
    } catch (err) {
      setError(err);
    } finally {
      // Báo cả số file đã lên trước khi file sau lỗi.
      setUploadedCount(done);
      setUploading(null);
      if (done > 0) onDone();
    }
  }

  const status =
    uploading || uploadedCount > 0 || error !== null ? (
      <>
        {uploading && <ProgressBar label={t("content.uploading", { names: uploading.join(", ") })} />}
        {uploadedCount > 0 && <Notice tone="success">{t("content.uploaded", { count: uploadedCount })}</Notice>}
        {error !== null && <Notice tone="danger">{contentUploadError(t, error)}</Notice>}
      </>
    ) : null;

  return {
    start: (files) => {
      if (uploading === null) void run(files);
    },
    busy: uploading !== null,
    status,
  };
}
