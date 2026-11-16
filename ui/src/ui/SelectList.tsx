import { Children, type ReactNode } from "react";

import { cx } from "./cx";
import styles from "./SelectList.module.css";

interface SelectListProps {
  /** Tên danh sách cho trình đọc màn hình. */
  label: string;
  /** Ô tìm kiếm, giãn hết phần còn lại của hàng trên cùng. */
  search?: ReactNode;
  /** Nút cạnh ô tìm kiếm, ví dụ "Thêm". */
  actions?: ReactNode;
  /** Dòng lọc dưới ô tìm kiếm, thường là `SegmentedControl`. */
  filters?: ReactNode;
  children: ReactNode;
}

/**
 * Danh sách chọn một mục để xem chi tiết bên cạnh (người chơi…), gọn hơn `DataTable`: mỗi hàng là
 * một nút có ảnh, tên, dòng phụ và huy hiệu. Hàng chia nhóm bằng `SelectListGroup`.
 */
export function SelectList({ label, search, actions, filters, children }: SelectListProps) {
  return (
    <section className={styles.list} aria-label={label}>
      {(search || actions) && (
        <div className={styles.toolbar}>
          {search && <div className={styles.search}>{search}</div>}
          {actions}
        </div>
      )}
      {filters && <div className={styles.filters}>{filters}</div>}
      {children}
    </section>
  );
}

interface SelectListGroupProps {
  title: string;
  /** Số mục, hiện mờ cạnh tiêu đề. */
  count?: number;
  /** Hiện khi nhóm không có mục nào. */
  empty?: ReactNode;
  children?: ReactNode;
}

export function SelectListGroup({ title, count, empty, children }: SelectListGroupProps) {
  const hasItems = Children.toArray(children).length > 0;
  return (
    <div className={styles.group}>
      <h2 className={styles.groupTitle}>
        {title}
        {count !== undefined && <span className={styles.count}>{count}</span>}
      </h2>
      {hasItems ? <ul className={styles.items}>{children}</ul> : empty && <p className={styles.empty}>{empty}</p>}
    </div>
  );
}

interface SelectListItemProps {
  /** Ảnh đầu hàng, thường là `Avatar`. */
  leading?: ReactNode;
  title: string;
  subtitle?: ReactNode;
  /** Huy hiệu cuối hàng. */
  trailing?: ReactNode;
  selected?: boolean;
  onSelect: () => void;
}

export function SelectListItem({ leading, title, subtitle, trailing, selected = false, onSelect }: SelectListItemProps) {
  return (
    <li>
      <button type="button" className={cx(styles.item, selected && styles.selected)} aria-pressed={selected} onClick={onSelect}>
        {leading}
        <span className={styles.body}>
          <span className={styles.title}>{title}</span>
          {subtitle && <span className={styles.subtitle}>{subtitle}</span>}
        </span>
        {trailing && <span className={styles.trailing}>{trailing}</span>}
      </button>
    </li>
  );
}
