import { Check } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cx } from "./cx";
import styles from "./progress.module.css";

interface StepsProps {
  /** Nhãn từng bước, đã dịch. */
  steps: string[];
  /** Bước đang làm, tính từ 0. */
  current: number;
}

/** Các bước của wizard: bước đã qua có dấu tích, bước hiện tại được tô màu. */
export function Steps({ steps, current }: StepsProps) {
  const { t } = useTranslation();
  return (
    <ol className={styles.steps} aria-label={t("ui.stepOf", { current: current + 1, total: steps.length })}>
      {steps.map((label, index) => {
        const done = index < current;
        const active = index === current;
        return (
          <li
            key={label}
            className={cx(styles.step, done && styles.done, active && styles.active)}
            aria-current={active ? "step" : undefined}
          >
            <span className={styles.index} aria-hidden>
              {done ? <Check size={14} /> : index + 1}
            </span>
            <span className={styles.stepLabel}>{label}</span>
          </li>
        );
      })}
    </ol>
  );
}

interface ProgressBarProps {
  label: string;
  /** 0–100. Bỏ trống khi chưa biết còn bao lâu: thanh chạy liên tục. */
  value?: number;
  tone?: "primary" | "success" | "danger";
}

export function ProgressBar({ label, value, tone = "primary" }: ProgressBarProps) {
  const { i18n } = useTranslation();
  const known = value !== undefined;
  const clamped = known ? Math.min(Math.max(value, 0), 100) : 0;
  const percent = new Intl.NumberFormat(i18n.language, { style: "percent" }).format(clamped / 100);

  return (
    <div className={styles.progress}>
      <div className={styles.progressText}>
        <span>{label}</span>
        {known && <span className={styles.percent}>{percent}</span>}
      </div>
      <div
        className={cx(styles.bar, styles[tone])}
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={known ? clamped : undefined}
      >
        <span className={cx(styles.fill, !known && styles.indeterminate)} style={known ? { width: `${clamped}%` } : undefined} />
      </div>
    </div>
  );
}
