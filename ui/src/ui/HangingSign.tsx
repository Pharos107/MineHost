import type { ReactNode } from "react";

import styles from "./HangingSign.module.css";

interface HangingSignProps {
  /** Logo hoặc hình ở đầu bảng (ví dụ `BlockWordmark`). */
  logo?: ReactNode;
  title: string;
  /** Cấp tiêu đề (`h1` khi bảng là nội dung chính của trang). */
  titleAs?: "h1" | "h2";
  description?: string;
  /** Chân bảng ngăn bằng đường kẻ (thao tác phụ, hướng dẫn). */
  footer?: ReactNode;
  children?: ReactNode;
}

/**
 * Bảng hiệu gỗ treo bằng hai sợi xích như hanging sign trong Minecraft, cho trang độc lập như
 * Đăng nhập. Ban ngày là gỗ sồi, ban đêm là gỗ vân sam; hai lớp gỗ mờ dần sang nhau khi đổi giao diện.
 */
export function HangingSign({ logo, title, titleAs: Heading = "h2", description, footer, children }: HangingSignProps) {
  return (
    <section className={styles.sign}>
      <span className={styles.chainLeft} aria-hidden />
      <span className={styles.chainRight} aria-hidden />
      <div className={styles.board}>
        <div className={styles.content}>
          <header className={styles.header}>
            {logo}
            <Heading className={styles.title}>{title}</Heading>
            {description && <p className={styles.description}>{description}</p>}
          </header>
          {children}
          {footer && <footer className={styles.footer}>{footer}</footer>}
        </div>
      </div>
    </section>
  );
}
