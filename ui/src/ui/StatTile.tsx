import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

import { cx } from "./cx";
import styles from "./StatTile.module.css";

/** Từ mức này thanh đổi sang màu cảnh báo. */
const METER_HIGH = 90;

interface StatTileProps {
  label: string;
  /** Số chính, đã định dạng theo ngôn ngữ (ví dụ "3,2"). Chưa có số thì để "—". */
  value: string;
  /** Đơn vị nhỏ sau số, ví dụ "GB", "%". */
  unit?: string;
  /** Dòng phụ dưới số, ví dụ "trên 8 GB đã đặt". */
  detail?: ReactNode;
  icon?: LucideIcon;
  /** 0–100: vẽ thanh mức dưới số, ví dụ RAM đã dùng so với mức đặt. */
  meter?: number;
}

/** Một con số đứng riêng (Trang chủ, Tổng quan). Không có biểu đồ nên không cần tooltip. */
export function StatTile({ label, value, unit, detail, icon: Icon, meter }: StatTileProps) {
  const level = meter === undefined ? undefined : Math.min(Math.max(meter, 0), 100);
  return (
    <div className={styles.tile}>
      <p className={styles.label}>
        {Icon && <Icon size={16} aria-hidden />}
        {label}
      </p>
      <p className={styles.value}>
        {value}
        {unit && <span className={styles.unit}>{unit}</span>}
      </p>
      {level !== undefined && (
        <div className={styles.meter} role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(level)}>
          <span className={cx(styles.meterFill, level >= METER_HIGH && styles.meterHigh)} style={{ width: `${level}%` }} />
        </div>
      )}
      {detail && <p className={styles.detail}>{detail}</p>}
    </div>
  );
}

/** Lưới các `StatTile`, tự xuống hàng và giãn đủ chiều ngang. */
export function StatGrid({ children }: { children: ReactNode }) {
  return <div className={styles.grid}>{children}</div>;
}
