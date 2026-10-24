import { Ellipsis, type LucideIcon } from "lucide-react";
import { useEffect, useId, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";

import { IconButton, type ButtonSize } from "./Button";
import { cx } from "./cx";
import styles from "./Menu.module.css";

export interface MenuItem {
  label: string;
  icon?: LucideIcon;
  onSelect: () => void;
  tone?: "danger";
  disabled?: boolean;
}

interface MenuProps {
  /** Nhãn của nút mở menu, ví dụ "Thao tác khác". */
  label: string;
  items: MenuItem[];
  icon?: LucideIcon;
  size?: ButtonSize;
}

/** Menu thả xuống cho các thao tác ít dùng. Esc, Tab, bấm ra ngoài hoặc cuộn trang thì đóng. */
export function Menu({ label, items, icon = Ellipsis, size = "md" }: MenuProps) {
  const [open, setOpen] = useState(false);
  const menuId = useId();
  const root = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    if (open) placeBelowOrAbove(root.current?.querySelector("button"), list.current);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    list.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
    const onPointerDown = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    // Menu đặt theo vị trí cố định trên màn hình, cuộn hay đổi cỡ là lệch khỏi nút nên đóng luôn.
    const onMove = () => setOpen(false);
    document.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("scroll", onMove, true);
    window.addEventListener("resize", onMove);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("scroll", onMove, true);
      window.removeEventListener("resize", onMove);
    };
  }, [open]);

  function close(returnFocus: boolean) {
    setOpen(false);
    if (returnFocus) root.current?.querySelector<HTMLButtonElement>("button")?.focus();
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close(true);
    } else if (e.key === "Tab") {
      close(false);
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      moveFocus(list.current, e.key === "ArrowDown" ? 1 : -1);
    }
  }

  return (
    <div className={styles.root} ref={root}>
      <IconButton
        icon={icon}
        label={label}
        size={size}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        onClick={() => setOpen((value) => !value)}
      />
      {open && (
        <div className={styles.menu} id={menuId} role="menu" aria-label={label} ref={list} onKeyDown={onKeyDown}>
          {items.map(({ label: itemLabel, icon: Icon, onSelect, tone, disabled }) => (
            <button
              key={itemLabel}
              type="button"
              role="menuitem"
              disabled={disabled}
              className={cx(styles.item, tone === "danger" && styles.danger)}
              onClick={() => {
                close(true);
                onSelect();
              }}
            >
              {Icon && <Icon size={16} aria-hidden />}
              {itemLabel}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * Đặt menu theo `position: fixed` cạnh nút, để không bị cắt khi nằm trong vùng cuộn (bảng).
 * Không đủ chỗ phía dưới thì mở lên trên.
 */
function placeBelowOrAbove(trigger: HTMLElement | null | undefined, menu: HTMLElement | null) {
  if (!trigger || !menu) return;
  const gap = 4;
  // clientWidth/Height không tính thanh cuộn, khớp với gốc tọa độ của `position: fixed`.
  const { clientWidth, clientHeight } = document.documentElement;
  const rect = trigger.getBoundingClientRect();
  const fitsBelow = rect.bottom + gap + menu.offsetHeight <= clientHeight;
  menu.style.top = `${fitsBelow ? rect.bottom + gap : Math.max(gap, rect.top - gap - menu.offsetHeight)}px`;
  menu.style.right = `${Math.max(gap, clientWidth - rect.right)}px`;
}

function moveFocus(container: HTMLElement | null, step: 1 | -1) {
  const items = Array.from(container?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []);
  if (items.length === 0) return;
  const current = items.indexOf(document.activeElement as HTMLButtonElement);
  items[(current + step + items.length) % items.length]?.focus();
}
