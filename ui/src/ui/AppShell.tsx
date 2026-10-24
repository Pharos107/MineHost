import { Blocks } from "lucide-react";
import type { ReactNode } from "react";

import styles from "./AppShell.module.css";

interface AppShellProps {
  /** Nút ở bên phải thanh trên: đổi ngôn ngữ, Thoát. */
  actions?: ReactNode;
  children: ReactNode;
}

/** Khung chung của mọi trang: thanh trên cố định và vùng nội dung có độ rộng tối đa. */
export function AppShell({ actions, children }: AppShellProps) {
  return (
    <div className={styles.shell}>
      <header className={styles.header}>
        <div className={styles.bar}>
          <span className={styles.brand}>
            <Blocks size={22} aria-hidden />
            MineHost
          </span>
          {actions && <div className={styles.actions}>{actions}</div>}
        </div>
      </header>
      <main className={styles.main}>{children}</main>
    </div>
  );
}
