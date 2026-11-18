import type { LucideIcon } from "lucide-react";
import type { ButtonHTMLAttributes, ReactNode } from "react";

import styles from "./Button.module.css";
import { cx } from "./cx";
import { Spinner } from "./Spinner";

export type ButtonVariant = "primary" | "secondary" | "danger" | "ghost";
export type ButtonSize = "sm" | "md" | "lg";

type NativeProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "className" | "style" | "children">;

interface ButtonProps extends NativeProps {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** Trải nút hết chiều ngang khung chứa. */
  fullWidth?: boolean;
  /** `start`: chữ dồn trái, dùng cho nút là một dòng trong danh sách (ví dụ chọn thư mục). */
  align?: "center" | "start";
  /** Đang chờ kết quả: hiện vòng quay và không cho bấm lại. */
  loading?: boolean;
  icon?: LucideIcon;
  children: ReactNode;
}

export function Button({
  variant = "secondary",
  size = "md",
  fullWidth = false,
  align = "center",
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
      className={cx(styles.button, styles[variant], styles[size], fullWidth && styles.fullWidth, align === "start" && styles.alignStart)}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
    >
      {loading ? <Spinner /> : Icon && <Icon size={size === "lg" ? 18 : 16} aria-hidden />}
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
  /** Icon màu đỏ trên nền trong suốt: thao tác phá hủy nằm giữa các nút thường (ví dụ trong bảng). */
  tone?: "danger";
  /** Đang bật (ví dụ bộ lọc tìm kiếm đang kích hoạt). */
  active?: boolean;
}

export function IconButton({ icon: Icon, label, variant = "ghost", size = "md", tone, active, type = "button", ...rest }: IconButtonProps) {
  const iconSize = size === "sm" ? 14 : size === "lg" ? 20 : 18;
  return (
    <button
      {...rest}
      type={type}
      className={cx(styles.button, styles.iconOnly, styles[variant], styles[size], tone === "danger" && styles.dangerText, active && styles.active)}
      aria-label={label}
      title={label}
    >
      <Icon size={iconSize} aria-hidden />
    </button>
  );
}
