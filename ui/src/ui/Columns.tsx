import type { ReactNode } from "react";

import styles from "./Columns.module.css";

interface ColumnsProps {
  /** 2: chia đôi trên màn hình rộng, màn hình hẹp vẫn xếp một cột. 1: luôn một cột. */
  count: 1 | 2;
  children: ReactNode;
}

/** Xếp các thẻ thành một hoặc hai cột bằng nhau (ví dụ hai biểu đồ ở Tổng quan). */
export function Columns({ count, children }: ColumnsProps) {
  return (
    <div className={count === 2 ? styles.two : styles.one} data-columns={count}>
      {children}
    </div>
  );
}
