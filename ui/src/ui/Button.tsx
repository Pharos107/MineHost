import type { LucideIcon } from "lucide-react";
import type { ButtonHTMLAttributes, ReactNode } from "react";

import styles from "./Button.module.css";
import { cx } from "./cx";
import { Spinner } from "./Spinner";

export type ButtonVariant = "primary" | "secondary" | "danger" | "ghost";
export type ButtonSize = "sm" | "md";

type NativeProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "className" | "style" | "children">;

interface ButtonProps extends NativeProps {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** Đang chờ kết quả: hiện vòng quay và không cho bấm lại. */
  loading?: boolean;
  icon?: LucideIcon;
  children: ReactNode;
}

export function Button({
  variant = "secondary",
  size = "md",
  loading = false,
  icon: Icon,
  disabled,
  type = "button",
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      {...rest}
      type={type}
      className={cx(styles.button, styles[variant], styles[size])}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
    >
      {loading ? <Spinner /> : Icon && <Icon size={16} aria-hidden />}
      {children}
    </button>
  );
}

interface IconButtonProps extends NativeProps {
  icon: LucideIcon;
  /** Bắt buộc: nút chỉ có icon thì trình đọc màn hình đọc nhãn này. */
  label: string;
  variant?: ButtonVariant;
  size?: ButtonSize;
}

export function IconButton({ icon: Icon, label, variant = "ghost", size = "md", type = "button", ...rest }: IconButtonProps) {
  return (
    <button
      {...rest}
      type={type}
      className={cx(styles.button, styles.iconOnly, styles[variant], styles[size])}
      aria-label={label}
      title={label}
    >
      <Icon size={size === "sm" ? 14 : 18} aria-hidden />
    </button>
  );
}
