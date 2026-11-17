import { X } from "lucide-react";
import { useEffect, useSyncExternalStore, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { IconButton } from "./Button";
import { Notice, type NoticeTone } from "./Notice";
import styles from "./Toast.module.css";

/** Thời gian một thông báo xong việc ở lại trên màn hình. */
export const TOAST_MS = 5000;

export interface ToastInput {
  tone?: NoticeTone;
  title?: string;
  /** Nội dung; có thể là component có hook (ví dụ thanh tiến độ của một Job). */
  body?: ReactNode;
  /** `true` khi việc còn chạy: không tự tắt cho tới khi `update` bỏ cờ này. */
  sticky?: boolean;
}

interface ToastItem extends ToastInput {
  id: number;
  /** Đổi mỗi lần cập nhật để hẹn giờ tắt chạy lại từ đầu. */
  version: number;
}

let items: ToastItem[] = [];
let nextId = 1;
const listeners = new Set<() => void>();

function emit(next: ToastItem[]) {
  items = next;
  for (const listener of listeners) listener();
}

/**
 * Thông báo nổi ở góc phải dưới (không đẩy nội dung trang). Gọi được từ bất kỳ đâu; `Toaster`
 * gắn một lần ở gốc app. Thông báo thường tự tắt sau 5 giây; `sticky` ở lại tới khi xong việc.
 */
export const toast = {
  show(input: ToastInput): number {
    const id = nextId++;
    emit([...items, { ...input, id, version: 0 }]);
    return id;
  },
  update(id: number, patch: ToastInput) {
    emit(items.map((item) => (item.id === id ? { ...item, ...patch, version: item.version + 1 } : item)));
  },
  dismiss(id: number) {
    emit(items.filter((item) => item.id !== id));
  },
  /** Đóng hết, ví dụ khi đăng xuất hoặc giữa hai test. */
  clear() {
    emit([]);
  },
};

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function Toaster() {
  const { t } = useTranslation();
  const list = useSyncExternalStore(subscribe, () => items);
  if (list.length === 0) return null;
  return (
    <section className={styles.stack} aria-label={t("ui.notifications")}>
      {list.map((item) => (
        <ToastCard key={item.id} item={item} />
      ))}
    </section>
  );
}

function ToastCard({ item }: { item: ToastItem }) {
  const { t } = useTranslation();
  useEffect(() => {
    if (item.sticky) return;
    const timer = window.setTimeout(() => toast.dismiss(item.id), TOAST_MS);
    return () => window.clearTimeout(timer);
  }, [item.id, item.sticky, item.version]);
  return (
    <div className={styles.toast}>
      <Notice
        tone={item.tone}
        title={item.title}
        action={<IconButton icon={X} label={t("ui.close")} size="sm" onClick={() => toast.dismiss(item.id)} />}
      >
        {item.body}
      </Notice>
    </div>
  );
}
