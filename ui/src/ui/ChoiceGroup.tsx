import type { LucideIcon } from "lucide-react";
import { useId, type ReactNode } from "react";

import styles from "./ChoiceGroup.module.css";

export interface Choice {
  value: string;
  label: string;
  description?: string;
  icon?: LucideIcon;
  /** Nhãn nhỏ cạnh tên, ví dụ `Badge` "Khuyên dùng" hoặc "Sắp có". */
  badge?: ReactNode;
  disabled?: boolean;
}

interface ChoiceGroupProps {
  /** Đã dịch; là `legend` của nhóm. */
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: Choice[];
}

/**
 * Chọn một trong vài lựa chọn lớn, mỗi lựa chọn là một thẻ. Bên dưới là radio thật nên phím
 * mũi tên và trình đọc màn hình hoạt động sẵn.
 */
export function ChoiceGroup({ label, value, onChange, options }: ChoiceGroupProps) {
  const name = useId();
  return (
    <fieldset className={styles.group}>
      <legend className={styles.legend}>{label}</legend>
      {options.map((option) => {
        const Icon = option.icon;
        return (
          <label key={option.value} className={styles.choice}>
            <input
              className={styles.input}
              type="radio"
              name={name}
              value={option.value}
              checked={value === option.value}
              disabled={option.disabled}
              onChange={() => onChange(option.value)}
            />
            <span className={styles.card}>
              <span className={styles.head}>
                {Icon && (
                  <span className={styles.icon} aria-hidden>
                    <Icon size={18} />
                  </span>
                )}
                <span className={styles.body}>
                  <span className={styles.title}>
                    {option.label}
                    {option.badge}
                  </span>
                  {option.description && <span className={styles.description}>{option.description}</span>}
                </span>
              </span>
            </span>
          </label>
        );
      })}
    </fieldset>
  );
}
