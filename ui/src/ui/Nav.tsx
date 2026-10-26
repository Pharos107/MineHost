import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { NavLink } from "react-router";

import { cx } from "./cx";
import styles from "./Nav.module.css";

interface NavItemProps {
  to: string;
  icon: LucideIcon;
  label: string;
  /**
   * Chỉ sáng khi đường dẫn trùng khớp hoàn toàn. Cần cho mục cha như Tổng quan `/servers/:id`,
   * nếu không nó sáng cùng lúc với Console `/servers/:id/console`.
   */
  end?: boolean;
}

/** Một mục trên thanh bên. Mục đang mở có `aria-current="page"` (react-router tự đặt). */
export function NavItem({ to, icon: Icon, label, end = false }: NavItemProps) {
  return (
    <NavLink to={to} end={end} className={({ isActive }) => cx(styles.item, isActive && styles.active)}>
      <Icon size={18} aria-hidden />
      <span className={styles.label}>{label}</span>
    </NavLink>
  );
}

interface NavSectionProps {
  /** Tiêu đề nhóm, ví dụ tên server đang mở. */
  title?: string;
  children: ReactNode;
}

/** Nhóm các `NavItem`, có tiêu đề tùy chọn. */
export function NavSection({ title, children }: NavSectionProps) {
  return (
    <div className={styles.section}>
      {title && <p className={styles.title}>{title}</p>}
      <div className={styles.items}>{children}</div>
    </div>
  );
}
