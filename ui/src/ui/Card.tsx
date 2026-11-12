import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

import styles from "./Card.module.css";
import { cx } from "./cx";

interface CardProps {
  title?: string;
  description?: string;
  /** Biểu tượng nhỏ trước tiêu đề, giúp lướt nhanh giữa các mục (ví dụ trang Cài đặt). */
  icon?: LucideIcon;
  /** `split`: tiêu đề và mô tả bên trái, nội dung bên phải (màn hình rộng). Hợp với trang cài đặt. */
  layout?: "stack" | "split";
  /** Nút ở góc phải tiêu đề. */
  actions?: ReactNode;
  children?: ReactNode;
}

export function Card({ title, description, icon: Icon, layout = "stack", actions, children }: CardProps) {
  const hasHeader = title !== undefined || actions !== undefined;
  return (
    <section className={cx(styles.card, layout === "split" && styles.split)}>
      {hasHeader && (
        <header className={styles.header}>
          <div>
            {title && (
              <h2 className={styles.title}>
                {Icon && (
                  <span className={styles.icon}>
                    <Icon size={18} aria-hidden />
                  </span>
                )}
                {title}
              </h2>
            )}
            {description && <p className={styles.description}>{description}</p>}
          </div>
          {actions && <div className={styles.actions}>{actions}</div>}
        </header>
      )}
      {layout === "split" ? <div className={styles.content}>{children}</div> : children}
    </section>
  );
}

interface PageHeaderProps {
  title: string;
  description?: ReactNode;
  actions?: ReactNode;
}

/** Đầu trang dùng chung: mọi trang có đúng một `h1` ở đây. */
export function PageHeader({ title, description, actions }: PageHeaderProps) {
  return (
    <header className={styles.page}>
      <div className={styles.pageText}>
        <h1 className={styles.pageTitle}>{title}</h1>
        {description && <div className={styles.description}>{description}</div>}
      </div>
      {actions && <div className={styles.actions}>{actions}</div>}
    </header>
  );
}
