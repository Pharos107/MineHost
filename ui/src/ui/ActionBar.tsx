import type { ReactNode } from "react";

import styles from "./ActionBar.module.css";
import { cx } from "./cx";

interface ActionBarProps {
  /** Làm nổi bật thanh khi form có thay đổi chưa lưu. */
  dirty?: boolean;
  /** Trạng thái ngắn bên trái (ví dụ số thay đổi chưa lưu hoặc "Đã lưu."). */
  status?: ReactNode;
  children: ReactNode;
}

/** Thanh hành động dính ở đáy khung nhìn cho form dài (Cài đặt game…). */
export function ActionBar({ dirty = false, status, children }: ActionBarProps) {
  return (
    <div className={cx(styles.bar, dirty && styles.dirty)}>
      {status !== undefined && <div className={cx(styles.status, dirty && styles.statusDirty)}>{status}</div>}
      <div className={styles.actions}>{children}</div>
    </div>
  );
}
