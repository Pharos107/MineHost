import { ChevronRight } from "lucide-react";
import { Link } from "react-router";

import styles from "./Breadcrumb.module.css";

export interface Crumb {
  label: string;
  /** Có thì bấm được; mục cuối (đang ở đó) để trống. */
  to?: string;
}

/** Đường dẫn dạng "Gốc › world › region". Mục cuối có `aria-current="page"`. */
export function Breadcrumb({ items, label }: { items: Crumb[]; label: string }) {
  return (
    <nav aria-label={label}>
      <ol className={styles.list}>
        {items.map((item, i) => {
          const last = i === items.length - 1;
          return (
            <li key={`${i}-${item.label}`} className={styles.item}>
              {i > 0 && <ChevronRight size={14} aria-hidden className={styles.separator} />}
              {item.to && !last ? (
                <Link to={item.to} className={styles.link}>
                  {item.label}
                </Link>
              ) : (
                <span className={styles.current} aria-current={last ? "page" : undefined}>
                  {item.label}
                </span>
              )}
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
