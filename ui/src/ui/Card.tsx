import type { ReactNode } from "react";

import styles from "./Card.module.css";

interface CardProps {
  title?: string;
  description?: string;
  /** Nút ở góc phải tiêu đề. */
  actions?: ReactNode;
  children?: ReactNode;
}

export function Card({ title, description, actions, children }: CardProps) {
  const hasHeader = title !== undefined || actions !== undefined;
  return (
    <section className={styles.card}>
      {hasHeader && (
        <header className={styles.header}>
          <div>
            {title && <h2 className={styles.title}>{title}</h2>}
            {description && <p className={styles.description}>{description}</p>}
          </div>
          {actions && <div className={styles.actions}>{actions}</div>}
        </header>
      )}
      {children}
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
