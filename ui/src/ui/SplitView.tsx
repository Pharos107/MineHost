import { ArrowLeft } from "lucide-react";
import type { ReactNode } from "react";

import { Button } from "./Button";
import { cx } from "./cx";
import styles from "./SplitView.module.css";

interface SplitViewProps {
  list: ReactNode;
  detail: ReactNode;
  /** Màn hình hẹp: `true` thì chi tiết thay chỗ danh sách. Màn hình rộng luôn hiện cả hai. */
  detailOpen: boolean;
  /** Nút quay lại danh sách, chỉ hiện trên màn hình hẹp khi đang xem chi tiết. */
  backLabel: string;
  onBack: () => void;
}

/** Danh sách bên trái, chi tiết bên phải (dính khi cuộn) từ 1024 px; hẹp hơn thì một cột. */
export function SplitView({ list, detail, detailOpen, backLabel, onBack }: SplitViewProps) {
  return (
    <div className={cx(styles.split, detailOpen && styles.detailOpen)}>
      <div className={styles.list}>{list}</div>
      <div className={styles.detail}>
        <div className={styles.back}>
          <Button variant="ghost" size="sm" icon={ArrowLeft} onClick={onBack}>
            {backLabel}
          </Button>
        </div>
        {detail}
      </div>
    </div>
  );
}
