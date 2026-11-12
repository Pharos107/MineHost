import { Menu as MenuIcon, X } from "lucide-react";
import { useEffect, useId, useState, type MouseEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import styles from "./AppShell.module.css";
import { BlockBanner } from "./BlockBanner";
import { IconButton } from "./Button";
import { cx } from "./cx";

interface AppShellProps {
  /** `top`: thanh trên (trang không có thanh bên, như wizard). `sidebar`: thanh bên trái. */
  variant?: "top" | "sidebar";
  /** `top`: nút ở bên phải thanh trên (đổi ngôn ngữ, Thoát). */
  actions?: ReactNode;
  /** `sidebar`: các `NavSection`/`NavItem`. */
  nav?: ReactNode;
  /** `sidebar`: nhãn cho trình đọc màn hình của vùng điều hướng. */
  navLabel?: string;
  /** `sidebar`: chân thanh bên (đổi ngôn ngữ, Thoát). */
  footer?: ReactNode;
  children: ReactNode;
}

/** Khung chung của mọi trang: thanh trên hoặc thanh bên, và vùng nội dung có độ rộng tối đa. */
export function AppShell({ variant = "top", actions, nav, navLabel, footer, children }: AppShellProps) {
  if (variant === "sidebar") {
    return (
      <SidebarShell nav={nav} navLabel={navLabel} footer={footer}>
        {children}
      </SidebarShell>
    );
  }
  return (
    <div className={styles.shell}>
      <header className={styles.header}>
        <div className={styles.bar}>
          <Brand />
          {actions && <div className={styles.actions}>{actions}</div>}
        </div>
      </header>
      <main className={styles.main}>{children}</main>
    </div>
  );
}

function Brand() {
  return (
    <span className={styles.brand}>
      <BlockBanner seed="MineHost" variant="logo" />
      MineHost
    </span>
  );
}

type SidebarShellProps = Pick<AppShellProps, "nav" | "navLabel" | "footer" | "children">;

/**
 * Màn hình rộng: thanh bên luôn hiện. Màn hình hẹp: thanh bên ẩn thành ngăn kéo, mở bằng nút menu,
 * đóng khi chọn một mục, bấm ra ngoài hoặc Esc.
 */
function SidebarShell({ nav, navLabel, footer, children }: SidebarShellProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const drawerId = useId();

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);

  // Bắt sự kiện ở cả vùng điều hướng để không phải truyền `onClick` vào từng mục.
  const closeOnLink = (e: MouseEvent) => {
    if ((e.target as Element).closest("a")) setOpen(false);
  };

  return (
    <div className={styles.sidebarShell}>
      <header className={styles.mobileBar}>
        <IconButton
          icon={MenuIcon}
          label={t("ui.openMenu")}
          aria-expanded={open}
          aria-controls={drawerId}
          onClick={() => setOpen(true)}
        />
        <Brand />
      </header>
      {open && <div className={styles.backdrop} aria-hidden onClick={() => setOpen(false)} />}
      <aside id={drawerId} className={cx(styles.sidebar, open && styles.open)}>
        <div className={styles.sidebarHead}>
          <Brand />
          <span className={styles.closeDrawer}>
            <IconButton icon={X} label={t("ui.closeMenu")} onClick={() => setOpen(false)} />
          </span>
        </div>
        <nav className={styles.nav} aria-label={navLabel} onClick={closeOnLink}>
          {nav}
        </nav>
        {footer && <div className={styles.footer}>{footer}</div>}
      </aside>
      <main className={cx(styles.main, styles.sidebarMain)}>{children}</main>
    </div>
  );
}
