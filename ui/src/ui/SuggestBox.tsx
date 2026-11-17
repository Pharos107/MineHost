import type { ReactNode } from "react";

import { cx } from "./cx";
import styles from "./SuggestBox.module.css";

export interface Suggestion {
  value: string;
  /** Dòng chính, font mono (ví dụ cách gõ lệnh). */
  label: string;
  /** Mô tả ngắn bên phải. */
  detail?: string;
}

interface SuggestBoxProps {
  /** `id` của danh sách; ô nhập trỏ tới bằng `aria-controls`. */
  id: string;
  /** Tên danh sách cho trình đọc màn hình. */
  label: string;
  items: Suggestion[];
  /** Vị trí đang chọn (mũi tên lên / xuống). */
  active: number;
  onPick: (value: string) => void;
  /** Ô nhập; danh sách hiện ngay bên dưới. */
  children: ReactNode;
}

/** Ô nhập kèm danh sách gợi ý thả xuống (gợi ý lệnh console). Bàn phím do trang xử lý trên ô nhập. */
export function SuggestBox({ id, label, items, active, onPick, children }: SuggestBoxProps) {
  return (
    <div className={styles.box}>
      {children}
      {items.length > 0 && (
        <ul id={id} className={styles.list} role="listbox" aria-label={label}>
          {items.map((item, i) => (
            <li
              key={item.value}
              id={`${id}-${i}`}
              role="option"
              aria-selected={i === active}
              className={cx(styles.item, i === active && styles.active)}
              // Giữ focus ở ô nhập khi bấm chuột vào gợi ý.
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => onPick(item.value)}
            >
              <span className={styles.label}>{item.label}</span>
              {item.detail && <span className={styles.detail}>{item.detail}</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
