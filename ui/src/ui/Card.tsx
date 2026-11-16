import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

import { BlockBanner } from "./BlockBanner";
import styles from "./Card.module.css";
import { cx } from "./cx";

interface CardProps {
  id?: string;
  title?: string;
  /** Cấp tiêu đề trong thẻ (`h1` cho trang chỉ có một thẻ, mặc định `h2`). */
  titleAs?: "h1" | "h2";
  description?: string;
  /** Biểu tượng nhỏ trước tiêu đề, giúp lướt nhanh giữa các mục (ví dụ trang Cài đặt). */
  icon?: LucideIcon;
  /** Chuỗi sinh dải hoa văn khối ở đầu thẻ. */
  bannerSeed?: string;
  /** `split`: tiêu đề và mô tả bên trái, nội dung bên phải (màn hình rộng). Hợp với trang cài đặt. */
  layout?: "stack" | "split";
  /** Nút ở góc phải tiêu đề. */
  actions?: ReactNode;
  children?: ReactNode;
}

function CardHeader({ title, titleAs: Heading = "h2", description, icon: Icon, actions }: Pick<CardProps, "title" | "titleAs" | "description" | "icon" | "actions">) {
  return (
    <header className={styles.header}>
      <div>
        {title && (
          <Heading className={styles.title}>
            {Icon && (
              <span className={styles.icon}>
                <Icon size={18} aria-hidden />
              </span>
            )}
            {title}
          </Heading>
        )}
        {description && <p className={styles.description}>{description}</p>}
      </div>
      {actions && <div className={styles.actions}>{actions}</div>}
    </header>
  );
}

export function Card({ id, title, titleAs = "h2", description, icon, bannerSeed, layout = "stack", actions, children }: CardProps) {
  const hasHeader = title !== undefined || actions !== undefined;
  const body = (
    <>
      {hasHeader && <CardHeader title={title} titleAs={titleAs} description={description} icon={icon} actions={actions} />}
      {layout === "split" ? <div className={styles.content}>{children}</div> : children}
    </>
  );

  return (
    <section id={id} className={cx(styles.card, bannerSeed ? styles.withBanner : layout === "split" && styles.split)}>
      {bannerSeed ? (
        <>
          <BlockBanner seed={bannerSeed} />
          <div className={cx(styles.body, layout === "split" && styles.split)}>{body}</div>
        </>
      ) : (
        body
      )}
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
