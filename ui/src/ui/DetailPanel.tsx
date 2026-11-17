import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

import { cx } from "./cx";
import styles from "./DetailPanel.module.css";

interface DetailHeaderProps {
  title: string;
  /** Ảnh lớn bên trái (skin 3D, biểu tượng). Màn hình hẹp thì nằm trên. */
  media?: ReactNode;
  /** Dòng nhỏ dưới tên, ví dụ "Lần cuối vào 2 giờ trước". */
  subtitle?: ReactNode;
  /** Huy hiệu, thông tin thêm dưới dòng phụ. */
  children?: ReactNode;
}

/** Đầu khung chi tiết của một mục chọn từ danh sách (người chơi…): ảnh, tên `h2`, thông tin. */
export function DetailHeader({ title, media, subtitle, children }: DetailHeaderProps) {
  return (
    <header className={styles.header}>
      {media && <div className={styles.media}>{media}</div>}
      <div className={styles.headerBody}>
        <div>
          <h2 className={styles.title}>{title}</h2>
          {subtitle && <p className={styles.subtitle}>{subtitle}</p>}
        </div>
        {children}
      </div>
    </header>
  );
}

interface DetailSectionProps {
  title: string;
  icon?: LucideIcon;
  /** `danger`: tiêu đề đỏ, cho nhóm thao tác khó hoàn tác (đuổi, cấm). */
  tone?: "neutral" | "danger";
  children: ReactNode;
}

/** Một nhóm thao tác trong khung chi tiết, ngăn với nhóm trên bằng đường kẻ mảnh. */
export function DetailSection({ title, icon: Icon, tone = "neutral", children }: DetailSectionProps) {
  return (
    <section className={cx(styles.section, tone === "danger" && styles.danger)}>
      <h3 className={styles.sectionTitle}>
        {Icon && <Icon size={16} aria-hidden />}
        {title}
      </h3>
      {children}
    </section>
  );
}
