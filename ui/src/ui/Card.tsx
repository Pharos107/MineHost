import type { LucideIcon } from "lucide-react";
import { useRef, type ReactNode } from "react";

import { BlockBanner } from "./BlockBanner";
import styles from "./Card.module.css";
import { cx } from "./cx";

export interface RectBox {
  left: number;
  top: number;
  width: number;
  height: number;
}

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
  /** `subtle`: khung nền phụ cho tóm tắt / hành động lồng trong thẻ; `dashed`: vùng tải file gọn. */
  variant?: "default" | "subtle" | "dashed";
  /** Nút ở góc phải tiêu đề. */
  actions?: ReactNode;
  /** Cho phép bấm cả thẻ (trừ nút trong `actions`) để mở chi tiết. */
  onClick?: (rect?: RectBox) => void;
  /** Nhãn cho nút tiêu đề bàn phím khi thẻ bấm được. */
  clickLabel?: string;
  children?: ReactNode;
}

function CardHeader({
  title,
  titleAs: Heading = "h2",
  description,
  icon: Icon,
  actions,
  onTitleClick,
  clickLabel,
}: Pick<CardProps, "title" | "titleAs" | "description" | "icon" | "actions" | "clickLabel"> & {
  onTitleClick?: () => void;
}) {
  const titleInner = (
    <>
      {Icon && (
        <span className={styles.icon}>
          <Icon size={18} aria-hidden />
        </span>
      )}
      {title}
    </>
  );

  return (
    <header className={styles.header}>
      <div>
        {title && (
          <Heading className={styles.title}>
            {onTitleClick ? (
              <button type="button" className={styles.titleTrigger} aria-label={clickLabel ?? title} onClick={onTitleClick}>
                {titleInner}
              </button>
            ) : (
              titleInner
            )}
          </Heading>
        )}
        {description && <p className={styles.description}>{description}</p>}
      </div>
      {actions && <div className={styles.actions}>{actions}</div>}
    </header>
  );
}

export function Card({
  id,
  title,
  titleAs = "h2",
  description,
  icon,
  bannerSeed,
  layout = "stack",
  variant = "default",
  actions,
  onClick,
  clickLabel,
  children,
}: CardProps) {
  const cardRef = useRef<HTMLElement>(null);
  const triggerClick = () => {
    const rect = cardRef.current?.getBoundingClientRect();
    onClick?.(rect ? { left: rect.left, top: rect.top, width: rect.width, height: rect.height } : undefined);
  };
  const hasHeader = title !== undefined || actions !== undefined;
  const body = (
    <>
      {hasHeader && (
        <CardHeader
          title={title}
          titleAs={titleAs}
          description={description}
          icon={icon}
          actions={actions}
          onTitleClick={onClick ? triggerClick : undefined}
          clickLabel={clickLabel}
        />
      )}
      {layout === "split" ? <div className={styles.content}>{children}</div> : children}
    </>
  );

  return (
    <section
      ref={cardRef}
      id={id}
      className={cx(
        styles.card,
        variant !== "default" && styles[variant],
        onClick && styles.interactive,
        bannerSeed ? styles.withBanner : layout === "split" && styles.split,
      )}
      onClick={
        onClick
          ? (e) => {
              const target = e.target as Element | null;
              if (target?.closest("button, a, input, select")) return;
              if (window.getSelection()?.toString()) return;
              triggerClick();
            }
          : undefined
      }
    >
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
  /** Dòng nhỏ phía trên tiêu đề, ví dụ loader và phiên bản của server. */
  eyebrow?: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
}

/** Đầu trang dùng chung: mọi trang có đúng một `h1` ở đây. */
export function PageHeader({ title, eyebrow, description, actions }: PageHeaderProps) {
  return (
    <header className={styles.page}>
      <div className={styles.pageText}>
        {eyebrow && <p className={styles.eyebrow}>{eyebrow}</p>}
        <h1 className={styles.pageTitle}>{title}</h1>
        {description && <div className={styles.description}>{description}</div>}
      </div>
      {actions && <div className={styles.actions}>{actions}</div>}
    </header>
  );
}
