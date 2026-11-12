import type { ReactNode } from "react";

import { BlockBanner } from "./BlockBanner";
import styles from "./ServerCard.module.css";

interface ServerCardProps {
  /** Id server: sinh hoa văn khối riêng ở đầu thẻ. */
  seed: string;
  /** Tên server, thường là một `Link` tới trang chi tiết. */
  title: ReactNode;
  /** Thường là `StatusBadge`. */
  status: ReactNode;
  /** Dòng nhỏ dưới tên, ví dụ "Paper 1.21.4, 4 GB RAM". */
  meta?: ReactNode;
  /** Các dòng thông tin: địa chỉ vào, người đang chơi. */
  children?: ReactNode;
  /** Nút ở chân thẻ. */
  actions?: ReactNode;
}

/** Thẻ của một server ở Trang chủ và trang Máy chủ: hoa văn riêng, trạng thái, cách vào, ai đang chơi. */
export function ServerCard({ seed, title, status, meta, children, actions }: ServerCardProps) {
  return (
    <article className={styles.card}>
      <BlockBanner seed={seed} />
      <div className={styles.body}>
        <header className={styles.header}>
          <div className={styles.heading}>
            <h2 className={styles.title}>{title}</h2>
            {meta && <p className={styles.meta}>{meta}</p>}
          </div>
          {status}
        </header>
        {children && <div className={styles.rows}>{children}</div>}
      </div>
      {actions && <footer className={styles.actions}>{actions}</footer>}
    </article>
  );
}

/** Lưới thẻ server, tự xuống hàng theo độ rộng. */
export function ServerCardGrid({ children }: { children: ReactNode }) {
  return <div className={styles.grid}>{children}</div>;
}
