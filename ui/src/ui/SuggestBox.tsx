import { useEffect, useRef, type ReactNode } from "react";

import { cx } from "./cx";
import styles from "./SuggestBox.module.css";

export interface Suggestion {
  value: string;
  /** Dòng chính, font mono (ví dụ cách gõ lệnh). */
  label: string;
  /** Mô tả ngắn bên cạnh. */
  detail?: string;
  /** Nhãn phân loại ngắn ở góc phải (Lệnh, Mod, Người chơi…). */
  tag?: string;
}

export interface SyntaxPart {
  text: string;
  active?: boolean;
  optional?: boolean;
}

interface CommandSyntaxProps {
  /** Tên lệnh đang gõ (có thể kèm `/` ở đầu). */
  command: string;
  parts: SyntaxPart[];
  description?: string;
}

/** Thanh hiển thị cú pháp trực tiếp trên ô nhập, tô đậm tham số đang điền. */
export function CommandSyntax({ command, parts, description }: CommandSyntaxProps) {
  return (
    <div className={styles.syntax} aria-live="polite">
      <div className={styles.syntaxTokens}>
        <span className={styles.syntaxCommand}>{command}</span>
        {parts.map((part, i) => (
          <span
            key={`${part.text}-${i}`}
            className={cx(styles.syntaxPart, part.optional && styles.syntaxOptional, part.active && styles.syntaxActive)}
          >
            {part.text}
          </span>
        ))}
      </div>
      {description && <span className={styles.syntaxDesc}>{description}</span>}
    </div>
  );
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
  /** Thanh cú pháp hiển thị ngay trên ô nhập, không bị danh sách thả đè lên. */
  syntax?: ReactNode;
  /** Ô nhập; danh sách hiện ngay phía trên. */
  children: ReactNode;
}

/** Ô nhập kèm danh sách gợi ý mở lên trên (gợi ý lệnh console). Bàn phím do trang xử lý trên ô nhập. */
export function SuggestBox({ id, label, items, active, onPick, syntax, children }: SuggestBoxProps) {
  const listRef = useRef<HTMLUListElement>(null);

  useEffect(() => {
    if (items.length === 0) return;
    const activeEl = listRef.current?.children[active] as HTMLElement | undefined;
    activeEl?.scrollIntoView?.({ block: "nearest" });
  }, [active, items.length]);

  return (
    <div className={styles.box}>
      {syntax}
      {children}
      {items.length > 0 && (
        <ul ref={listRef} id={id} className={styles.list} role="listbox" aria-label={label}>
          {items.map((item, i) => (
            <li
              key={`${item.value}-${i}`}
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
              {item.tag && <span className={styles.tag}>{item.tag}</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
