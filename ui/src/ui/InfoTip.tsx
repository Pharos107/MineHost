import { CircleAlert } from "lucide-react";
import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";

import styles from "./InfoTip.module.css";

interface InfoTipProps {
  /** Nhãn ngắn cho trình đọc màn hình, ví dụ tên khóa đang giải thích. */
  label: string;
  /** Nội dung giải thích hiện khi rê chuột hoặc focus vào biểu tượng. */
  text: string;
}

/** Biểu tượng chấm than nhỏ hiện giải thích nổi khi rê chuột hoặc focus, không bị cắt trong bảng cuộn. */
export function InfoTip({ label, text }: InfoTipProps) {
  const [open, setOpen] = useState(false);
  const tipId = useId();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const tipRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    if (open) placeTooltip(triggerRef.current, tipRef.current);
  }, [open, text]);

  useEffect(() => {
    if (!open) return;
    const close = () => setOpen(false);
    const onPointerDown = (e: PointerEvent) => {
      if (!triggerRef.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [open]);

  return (
    <span className={styles.root}>
      <button
        ref={triggerRef}
        type="button"
        className={styles.trigger}
        aria-label={label}
        aria-describedby={open ? tipId : undefined}
        onMouseEnter={() => setOpen(true)}
        onMouseLeave={() => setOpen(false)}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        onClick={() => setOpen(true)}
        onKeyDown={(e) => {
          if (e.key === "Escape") setOpen(false);
        }}
      >
        <CircleAlert size={14} aria-hidden />
      </button>
      {open && (
        <div ref={tipRef} id={tipId} role="tooltip" className={styles.tip}>
          {text}
        </div>
      )}
    </span>
  );
}

function placeTooltip(trigger: HTMLElement | null, tip: HTMLElement | null) {
  if (!trigger || !tip) return;
  const gap = 6;
  const edge = 8;
  const { clientWidth, clientHeight } = document.documentElement;
  const rect = trigger.getBoundingClientRect();
  const fitsAbove = rect.top - gap - tip.offsetHeight >= edge;
  const top = fitsAbove
    ? rect.top - gap - tip.offsetHeight
    : Math.min(clientHeight - edge - tip.offsetHeight, rect.bottom + gap);
  const left = Math.max(edge, Math.min(rect.left, clientWidth - edge - tip.offsetWidth));
  tip.style.top = `${Math.max(edge, top)}px`;
  tip.style.left = `${left}px`;
}
