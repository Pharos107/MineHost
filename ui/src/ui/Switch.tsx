import { useId } from "react";

import styles from "./Switch.module.css";

interface SwitchProps {
  label: string;
  description?: string;
  /** Ẩn chữ bên cạnh (ví dụ đặt trong ô bảng), vẫn giữ `aria-label` cho trình đọc màn hình. */
  hideLabel?: boolean;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

/** Công tắc bật/tắt một tùy chọn, nhãn nằm ngay bên cạnh nên không cần `Field`. */
export function Switch({ label, description, hideLabel = false, checked, onChange, disabled }: SwitchProps) {
  const descriptionId = useId();
  return (
    <label className={styles.switch}>
      <input
        className={styles.input}
        type="checkbox"
        role="switch"
        checked={checked}
        disabled={disabled}
        aria-label={hideLabel ? label : undefined}
        aria-describedby={!hideLabel && description ? descriptionId : undefined}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className={styles.track} aria-hidden>
        <span className={styles.thumb} />
      </span>
      {!hideLabel && (
        <span className={styles.text}>
          <span className={styles.label}>{label}</span>
          {description && (
            <span className={styles.description} id={descriptionId}>
              {description}
            </span>
          )}
        </span>
      )}
    </label>
  );
}
