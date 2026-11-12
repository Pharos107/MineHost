import type { LucideIcon } from "lucide-react";

import styles from "./SegmentedControl.module.css";
import { cx } from "./cx";

export interface Segment<V extends string> {
  value: V;
  label: string;
  icon?: LucideIcon;
}

interface SegmentedControlProps<V extends string> {
  /** Nhãn cho trình đọc màn hình của cả nhóm. */
  label: string;
  value: V;
  options: Segment<V>[];
  onChange: (value: V) => void;
}

/** Chọn một trong vài cách xem (ví dụ Thẻ / Bảng). Gọn hơn `ChoiceGroup`, đặt được trên thanh công cụ. */
export function SegmentedControl<V extends string>({ label, value, options, onChange }: SegmentedControlProps<V>) {
  return (
    <div className={styles.group} role="radiogroup" aria-label={label}>
      {options.map(({ value: option, label: text, icon: Icon }) => (
        <button
          key={option}
          type="button"
          role="radio"
          aria-checked={option === value}
          className={cx(styles.segment, option === value && styles.active)}
          onClick={() => onChange(option)}
        >
          {Icon && <Icon size={16} aria-hidden />}
          {text}
        </button>
      ))}
    </div>
  );
}
