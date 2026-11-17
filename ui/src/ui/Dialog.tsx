import { X } from "lucide-react";
import { useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Button, IconButton } from "./Button";
import type { RectBox } from "./Card";
import { cx } from "./cx";
import styles from "./Dialog.module.css";
import { Field } from "./Field";
import { TextInput } from "./inputs";

interface DialogProps {
  open: boolean;
  onClose: () => void;
  title: string;
  size?: "md" | "lg" | "xl";
  /** Vị trí thẻ nguồn để phóng to mượt khi mở. */
  originRect?: RectBox | null;
  /** `false` khi người dùng phải chọn một nút (ví dụ EULA): Esc và bấm ra ngoài không đóng. */
  dismissible?: boolean;
  children: ReactNode;
}

/**
 * Hộp thoại dựa trên `<dialog>` để trình duyệt lo khóa focus và nền mờ. Nội dung chỉ được
 * dựng khi mở, nên trạng thái bên trong (ô đã gõ…) tự xóa sau mỗi lần đóng.
 */
export function Dialog({ open, onClose, title, size = "md", originRect, dismissible = true, children }: DialogProps) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();

  useLayoutEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) {
      dialog.showModal();
      dialog.querySelector<HTMLElement>("[data-autofocus]")?.focus();
      if (originRect && typeof dialog.animate === "function" && !window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) {
        const dest = dialog.getBoundingClientRect();
        if (dest.width > 0 && dest.height > 0) {
          const dx = originRect.left + originRect.width / 2 - (dest.left + dest.width / 2);
          const dy = originRect.top + originRect.height / 2 - (dest.top + dest.height / 2);
          const sx = Math.max(0.25, originRect.width / dest.width);
          const sy = Math.max(0.25, originRect.height / dest.height);
          dialog.animate(
            [
              { transform: `translate(${dx}px, ${dy}px) scale(${sx}, ${sy})`, opacity: 0.5 },
              { transform: "translate(0, 0) scale(1, 1)", opacity: 1 },
            ],
            { duration: 200, easing: "cubic-bezier(0.2, 0.8, 0.2, 1)" },
          );
        }
      }
    }
    if (!open && dialog.open) dialog.close();
  }, [open, originRect]);

  return (
    <dialog
      ref={ref}
      className={cx(styles.dialog, size === "lg" && styles.lg, size === "xl" && styles.xl)}
      aria-labelledby={titleId}
      onCancel={(e) => {
        // Đóng theo prop `open`, không để trình duyệt tự đóng rồi lệch với state của React.
        e.preventDefault();
        if (dismissible) onClose();
      }}
      onClick={(e) => {
        if (dismissible && e.target === e.currentTarget) onClose();
      }}
    >
      {open && (
        <div className={styles.content}>
          <header className={styles.header}>
            <h2 className={styles.title} id={titleId}>
              {title}
            </h2>
            {dismissible && <IconButton icon={X} label={t("ui.close")} size="sm" onClick={onClose} />}
          </header>
          {children}
        </div>
      )}
    </dialog>
  );
}

export function DialogBody({ children }: { children: ReactNode }) {
  return <div className={styles.body}>{children}</div>;
}

export function DialogFooter({ children }: { children: ReactNode }) {
  return <footer className={styles.footer}>{children}</footer>;
}

interface ConfirmDialogProps {
  open: boolean;
  onCancel: () => void;
  onConfirm: () => void;
  title: string;
  children: ReactNode;
  confirmLabel: string;
  tone?: "primary" | "danger";
  /** Bắt gõ lại đúng chuỗi này (ví dụ tên server) mới cho xác nhận. */
  requireText?: string;
  /** Bắt tích ô có nhãn này (ví dụ "Tôi hiểu") mới cho xác nhận. */
  requireAck?: string;
  loading?: boolean;
}

/** Hỏi lại trước thao tác khó hoàn tác: Buộc dừng, Xóa, Thoát khi còn server chạy. */
export function ConfirmDialog({ open, onCancel, title, ...rest }: ConfirmDialogProps) {
  return (
    <Dialog open={open} onClose={onCancel} title={title} dismissible={!rest.loading}>
      <ConfirmForm onCancel={onCancel} {...rest} />
    </Dialog>
  );
}

function ConfirmForm({ onCancel, onConfirm, children, confirmLabel, tone = "danger", requireText, requireAck, loading }: Omit<ConfirmDialogProps, "open" | "title">) {
  const { t } = useTranslation();
  const [typed, setTyped] = useState("");
  const [acked, setAcked] = useState(false);
  const allowed = (requireText === undefined || typed === requireText) && (requireAck === undefined || acked);

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (allowed && !loading) onConfirm();
      }}
    >
      <DialogBody>
        {children}
        {requireText !== undefined && (
          <Field label={t("ui.typeToConfirm", { text: requireText })}>
            <TextInput
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              autoComplete="off"
              spellCheck={false}
              data-autofocus
            />
          </Field>
        )}
        {requireAck !== undefined && (
          <label className={styles.ack}>
            <input type="checkbox" checked={acked} onChange={(e) => setAcked(e.target.checked)} />
            {requireAck}
          </label>
        )}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" onClick={onCancel} disabled={loading}>
          {t("ui.cancel")}
        </Button>
        <Button type="submit" variant={tone} disabled={!allowed} loading={loading}>
          {confirmLabel}
        </Button>
      </DialogFooter>
    </form>
  );
}
