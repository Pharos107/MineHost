import { CircleAlert, CircleCheck, Info, TriangleAlert, type LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

import { cx } from "./cx";
import styles from "./Notice.module.css";

export type NoticeTone = "info" | "success" | "warning" | "danger";

const ICONS: Record<NoticeTone, LucideIcon> = {
  info: Info,
  success: CircleCheck,
  warning: TriangleAlert,
  danger: CircleAlert,
};

interface NoticeProps {
  tone?: NoticeTone;
  title?: string;
  children?: ReactNode;
  /** Nút đi kèm, ví dụ "Thử lại". */
  action?: ReactNode;
}

/** Hộp thông báo duy nhất của app: lỗi, cảnh báo, ghi chú đều dùng cái này. */
export function Notice({ tone = "info", title, children, action }: NoticeProps) {
  const Icon = ICONS[tone];
  return (
    <div className={cx(styles.notice, styles[tone])} role={tone === "danger" ? "alert" : "status"}>
      <Icon className={styles.icon} size={18} aria-hidden />
      <div className={styles.content}>
        {title && <p className={styles.title}>{title}</p>}
        {children && <div className={styles.body}>{children}</div>}
      </div>
      {action && <div className={styles.action}>{action}</div>}
    </div>
  );
}
