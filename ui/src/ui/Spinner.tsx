import { cx } from "./cx";
import styles from "./Spinner.module.css";

interface SpinnerProps {
  size?: "sm" | "md";
  /** Có nhãn thì hiện chữ bên cạnh và báo cho trình đọc màn hình; không có thì chỉ để trang trí. */
  label?: string;
}

export function Spinner({ size = "sm", label }: SpinnerProps) {
  const circle = <span className={cx(styles.spinner, styles[size])} aria-hidden />;
  if (!label) return circle;
  return (
    <span className={styles.labelled} role="status">
      {circle}
      {label}
    </span>
  );
}
