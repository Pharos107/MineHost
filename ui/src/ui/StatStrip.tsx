import type { LucideIcon } from "lucide-react";

import styles from "./StatStrip.module.css";

export interface StripItem {
  label: string;
  /** Đã định dạng, ví dụ "16 GB / 31 GB". */
  value: string;
  icon: LucideIcon;
}

/** Một dải số liệu gọn (máy đang chạy MineHost), cho trang mà số liệu chỉ là thông tin phụ. */
export function StatStrip({ label, items }: { label: string; items: StripItem[] }) {
  return (
    <section className={styles.strip} aria-label={label}>
      <h2 className={styles.heading}>{label}</h2>
      <dl className={styles.items}>
        {items.map(({ label: name, value, icon: Icon }) => (
          <div key={name} className={styles.item}>
            <dt className={styles.label}>
              <Icon size={15} aria-hidden />
              {name}
            </dt>
            <dd className={styles.value}>{value}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
