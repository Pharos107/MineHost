import { useId, useRef, type KeyboardEvent, type ReactNode } from "react";

import { cx } from "./cx";
import styles from "./Tabs.module.css";

export interface TabItem {
  id: string;
  label: string;
  panel: ReactNode;
}

interface TabsProps {
  /** Tên cả nhóm tab cho trình đọc màn hình. */
  label: string;
  items: TabItem[];
  value: string;
  onChange: (id: string) => void;
}

const MOVES: Record<string, (index: number, count: number) => number> = {
  ArrowRight: (i, n) => (i + 1) % n,
  ArrowLeft: (i, n) => (i - 1 + n) % n,
  Home: () => 0,
  End: (_, n) => n - 1,
};

/** Tab theo mẫu WAI-ARIA: phím mũi tên chuyển tab, chỉ tab đang chọn nằm trong vòng Tab. */
export function Tabs({ label, items, value, onChange }: TabsProps) {
  const baseId = useId();
  const buttons = useRef(new Map<string, HTMLButtonElement>());
  const active = items.find((item) => item.id === value) ?? items[0];

  function onKeyDown(e: KeyboardEvent) {
    const move = MOVES[e.key];
    const index = items.findIndex((item) => item.id === active?.id);
    const next = move ? items[move(index, items.length)] : undefined;
    if (!next) return;
    e.preventDefault();
    onChange(next.id);
    buttons.current.get(next.id)?.focus();
  }

  return (
    <div className={styles.tabs}>
      <div className={styles.list} role="tablist" aria-label={label} onKeyDown={onKeyDown}>
        {items.map((item) => {
          const selected = item.id === active?.id;
          return (
            <button
              key={item.id}
              ref={(node) => {
                if (node) buttons.current.set(item.id, node);
                else buttons.current.delete(item.id);
              }}
              type="button"
              role="tab"
              id={`${baseId}-tab-${item.id}`}
              aria-selected={selected}
              aria-controls={`${baseId}-panel`}
              tabIndex={selected ? 0 : -1}
              className={cx(styles.tab, selected && styles.selected)}
              onClick={() => onChange(item.id)}
            >
              {item.label}
            </button>
          );
        })}
      </div>
      {active && (
        <div role="tabpanel" id={`${baseId}-panel`} aria-labelledby={`${baseId}-tab-${active.id}`} className={styles.panel}>
          {active.panel}
        </div>
      )}
    </div>
  );
}
