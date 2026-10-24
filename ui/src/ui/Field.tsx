import { CircleAlert } from "lucide-react";
import { createContext, useContext, useId, type ReactNode } from "react";

import styles from "./Field.module.css";

interface FieldControl {
  id: string;
  describedBy: string | undefined;
  invalid: boolean;
}

const FieldContext = createContext<FieldControl | null>(null);

/** Ô nhập bên trong `Field` lấy `id` và `aria-*` từ đây; đứng một mình thì nhận `null`. */
export function useFieldControl(): FieldControl | null {
  return useContext(FieldContext);
}

interface FieldProps {
  label: string;
  hint?: ReactNode;
  /** Câu lỗi đã dịch. Có lỗi thì ô nhập được đánh dấu `aria-invalid`. */
  error?: string;
  children: ReactNode;
}

/** Nhãn, gợi ý và lỗi cho một ô nhập; tự nối `id` để trình đọc màn hình đọc đủ. */
export function Field({ label, hint, error, children }: FieldProps) {
  const id = useId();
  const hintId = hint ? `${id}-hint` : undefined;
  const errorId = error ? `${id}-error` : undefined;
  const describedBy = [errorId, hintId].filter(Boolean).join(" ") || undefined;

  return (
    <div className={styles.field}>
      <label className={styles.label} htmlFor={id}>
        {label}
      </label>
      <FieldContext.Provider value={{ id, describedBy, invalid: Boolean(error) }}>{children}</FieldContext.Provider>
      {error && (
        <p className={styles.error} id={errorId}>
          <CircleAlert size={14} aria-hidden />
          {error}
        </p>
      )}
      {hint && (
        <div className={styles.hint} id={hintId}>
          {hint}
        </div>
      )}
    </div>
  );
}
