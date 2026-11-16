import type { ReactNode } from "react";

import styles from "./Columns.module.css";

interface ColumnsProps {
  /** 2: chia đôi trên màn hình rộng. 3: ba cột trên màn hình rất rộng, hai cột khi vừa. Màn hình hẹp luôn một cột. */
  count: 1 | 2 | 3;
  children: ReactNode;
}

const CLASS = { 1: styles.one, 2: styles.two, 3: styles.three };

/** Xếp các thẻ thành một, hai hoặc ba cột bằng nhau (ví dụ các biểu đồ ở Tổng quan). */
export function Columns({ count, children }: ColumnsProps) {
  return (
    <div className={CLASS[count]} data-columns={count}>
      {children}
    </div>
  );
}
