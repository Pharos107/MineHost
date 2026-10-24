import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";

import { useFieldControl } from "./Field";
import styles from "./Slider.module.css";

interface SliderProps {
  value: number;
  onChange: (value: number) => void;
  min: number;
  max: number;
  step?: number;
  /** Giá trị nên chọn, hiện thành một vạch có chữ "đề xuất" dưới thanh trượt. */
  recommended?: number;
  /** Cách hiện số, ví dụ 4096 → "4 GB". */
  formatValue?: (value: number) => string;
  disabled?: boolean;
}

export function Slider({ value, onChange, min, max, step = 1, recommended, formatValue = String, disabled }: SliderProps) {
  const { t } = useTranslation();
  const field = useFieldControl();
  const text = formatValue(value);

  return (
    <div className={styles.slider}>
      <div className={styles.track}>
        <input
          id={field?.id}
          aria-describedby={field?.describedBy}
          className={styles.range}
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          disabled={disabled}
          aria-valuetext={text}
          onChange={(e) => onChange(Number(e.target.value))}
        />
        {recommended !== undefined && <Mark ratio={ratioOf(recommended, min, max)} text={t("ui.recommended", { value: formatValue(recommended) })} />}
      </div>
      <output className={styles.value} htmlFor={field?.id}>
        {text}
      </output>
    </div>
  );
}

function ratioOf(at: number, min: number, max: number): number {
  return max > min ? Math.min(Math.max((at - min) / (max - min), 0), 1) : 0;
}

/**
 * Vạch nằm đúng tâm nút kéo tại giá trị đó: tâm nút đi từ nửa nút đến hết thanh trừ nửa nút.
 * Gần hai đầu thì neo chữ vào mép để không tràn ra ngoài.
 */
function Mark({ ratio, text }: { ratio: number; text: string }) {
  const left = `calc(${ratio} * (100% - var(--thumb)) + var(--thumb) / 2)`;
  const shift = ratio < 0.15 ? "0%" : ratio > 0.85 ? "-100%" : "-50%";
  const labelStyle: CSSProperties = { transform: `translateX(${shift})` };
  return (
    <span className={styles.mark} style={{ left }}>
      <span className={styles.markLabel} style={labelStyle}>
        {text}
      </span>
    </span>
  );
}
