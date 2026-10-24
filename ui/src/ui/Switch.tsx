import { useId } from "react";

import styles from "./Switch.module.css";

interface SwitchProps {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

/** Công tắc bật/tắt một tùy chọn, nhãn nằm ngay bên cạnh nên không cần `Field`. */
export function Switch({ label, description, checked, onChange, disabled }: SwitchProps) {
  const descriptionId = useId();
  return (
    <label className={styles.switch}>
      <input
        className={styles.input}
        type="checkbox"
        role="switch"
        checked={checked}
        disabled={disabled}
        aria-describedby={description ? descriptionId : undefined}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className={styles.track} aria-hidden>
        <span className={styles.thumb} />
      </span>
      <span className={styles.text}>
        <span className={styles.label}>{label}</span>
        {description && (
          <span className={styles.description} id={descriptionId}>
            {description}
          </span>
        )}
      </span>
    </label>
  );
}
