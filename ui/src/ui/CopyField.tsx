import { Check, Copy } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { IconButton } from "./Button";
import styles from "./CopyField.module.css";

const RESET_MS = 2000;

interface CopyFieldProps {
  value: string;
  /** Nhãn của nút chép, ví dụ "Chép địa chỉ". Mặc định là "Chép". */
  label?: string;
}

/** Hiện một giá trị (địa chỉ, đường dẫn) kèm nút chép vào clipboard. */
export function CopyField({ value, label }: CopyFieldProps) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<"idle" | "copied" | "failed">("idle");
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
      setStatus("copied");
    } catch {
      // Trình duyệt chặn clipboard: báo để người dùng tự bôi đen và chép.
      setStatus("failed");
    }
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setStatus("idle"), RESET_MS);
  }

  const message = status === "copied" ? t("ui.copied") : status === "failed" ? t("ui.copyFailed") : "";

  return (
    <span className={styles.field}>
      <code className={styles.value}>{value}</code>
      <IconButton
        icon={status === "copied" ? Check : Copy}
        label={label ?? t("ui.copy")}
        size="sm"
        onClick={() => void copy()}
      />
      <span className={status === "failed" ? styles.failed : "visually-hidden"} aria-live="polite">
        {message}
      </span>
    </span>
  );
}
