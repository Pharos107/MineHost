import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

import styles from "./StatTile.module.css";

interface StatTileProps {
  label: string;
  /** Số chính, đã định dạng theo ngôn ngữ (ví dụ "3,2"). Chưa có số thì để "—". */
  value: string;
  /** Đơn vị nhỏ sau số, ví dụ "GB", "%". */
  unit?: string;
  /** Dòng phụ dưới số, ví dụ "trên 8 GB đã đặt". */
  detail?: ReactNode;
  icon?: LucideIcon;
}

/** Một con số đứng riêng (Trang chủ, Tổng quan). Không có biểu đồ nên không cần tooltip. */
export function StatTile({ label, value, unit, detail, icon: Icon }: StatTileProps) {
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
      {detail && <p className={styles.detail}>{detail}</p>}
    </div>
  );
}

/** Lưới các `StatTile`, tự xuống hàng theo độ rộng màn hình. */
export function StatGrid({ children }: { children: ReactNode }) {
  return <div className={styles.grid}>{children}</div>;
}
