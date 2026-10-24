import type { ReactNode } from "react";

import styles from "./Facts.module.css";

export interface Fact {
  label: string;
  value: ReactNode;
}

/** Danh sách "nhãn: giá trị" (thông tin máy, phiên bản, RAM…). Màn hình hẹp thì xếp dọc. */
export function Facts({ items }: { items: Fact[] }) {
  return (
    <dl className={styles.facts}>
      {items.map((item) => (
        <div className={styles.row} key={item.label}>
          <dt className={styles.label}>{item.label}</dt>
          <dd className={styles.value}>{item.value}</dd>
        </div>
      ))}
    </dl>
  );
}
