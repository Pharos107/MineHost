import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, Dialog, DialogBody, DialogFooter, Field, Notice, TextInput } from "../../../ui";

interface NameDialogProps {
  title: string;
  label: string;
  confirmLabel: string;
  initial?: string;
  loading: boolean;
  error?: string;
  onCancel: () => void;
  onConfirm: (name: string) => void;
}

/**
 * Hỏi một tên file hoặc thư mục (tạo mới, đổi tên). Chỉ chặn tên rỗng và tên có dấu `/` `\`
 * ở đây; mọi kiểm tra khác (tên thiết bị Windows, trùng tên…) do backend trả mã lỗi.
 */
export function NameDialog({ title, label, confirmLabel, initial = "", loading, error, onCancel, onConfirm }: NameDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial);
  const trimmed = name.trim();
  const invalid = trimmed === "" || /[\\/]/.test(trimmed);

  return (
    <Dialog open onClose={onCancel} title={title}>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (!invalid) onConfirm(trimmed);
        }}
      >
        <DialogBody>
          <Field label={label} error={trimmed !== "" && invalid ? t("files.badName") : undefined}>
            <TextInput value={name} autoFocus autoComplete="off" spellCheck={false} onChange={(e) => setName(e.target.value)} />
          </Field>
          {error && <Notice tone="danger">{error}</Notice>}
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={onCancel}>
            {t("ui.cancel")}
          </Button>
          <Button type="submit" variant="primary" disabled={invalid} loading={loading}>
            {confirmLabel}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
