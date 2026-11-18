import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, Dialog, DialogBody, DialogFooter, Field, Notice, Stack, TextInput } from "../../../ui";

interface FetchUrlDialogProps {
  loading: boolean;
  error?: string;
  onCancel: () => void;
  onConfirm: (url: string, filename?: string) => void;
}

function isHttpUrl(raw: string): boolean {
  try {
    const parsed = new URL(raw.trim());
    return (parsed.protocol === "http:" || parsed.protocol === "https:") && parsed.hostname !== "";
  } catch {
    return false;
  }
}

/** Hộp thoại nhập URL http(s) để tải trực tiếp file về thư mục đang mở trên server. */
export function FetchUrlDialog({ loading, error, onCancel, onConfirm }: FetchUrlDialogProps) {
  const { t } = useTranslation();
  const [url, setUrl] = useState("");
  const [filename, setFilename] = useState("");
  const trimmedUrl = url.trim();
  const badUrl = trimmedUrl !== "" && !isHttpUrl(trimmedUrl);
  const badName = filename.trim() !== "" && /[\\/]/.test(filename.trim());
  const disabled = !isHttpUrl(trimmedUrl) || badName;

  return (
    <Dialog open onClose={onCancel} title={t("files.fetch.title")}>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (!disabled) onConfirm(trimmedUrl, filename.trim() || undefined);
        }}
      >
        <DialogBody>
          <Stack gap={3}>
            <Field label={t("files.fetch.url")} error={badUrl ? t("files.fetch.badUrl") : undefined}>
              <TextInput
                value={url}
                autoFocus
                autoComplete="off"
                placeholder={t("files.fetch.urlPlaceholder")}
                onChange={(e) => setUrl(e.target.value)}
              />
            </Field>
            <Field
              label={t("files.fetch.filename")}
              hint={t("files.fetch.filenameHint")}
              error={badName ? t("files.badName") : undefined}
            >
              <TextInput value={filename} autoComplete="off" spellCheck={false} onChange={(e) => setFilename(e.target.value)} />
            </Field>
            {error && <Notice tone="danger">{error}</Notice>}
          </Stack>
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={onCancel}>
            {t("ui.cancel")}
          </Button>
          <Button type="submit" variant="primary" disabled={disabled} loading={loading}>
            {t("files.fetch.submit")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
