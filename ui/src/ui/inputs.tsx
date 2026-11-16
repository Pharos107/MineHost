import { ChevronDown, Eye, EyeOff, Search } from "lucide-react";
import { useState, type InputHTMLAttributes, type SelectHTMLAttributes, type TextareaHTMLAttributes } from "react";
import { useTranslation } from "react-i18next";

import { cx } from "./cx";
import { useFieldControl } from "./Field";
import styles from "./Field.module.css";

type TextInputProps = Omit<InputHTMLAttributes<HTMLInputElement>, "className" | "style" | "type" | "size"> & {
  type?: "text" | "number" | "search" | "password" | "time";
  size?: "md" | "lg";
};

/** Ô nhập chữ. Đặt trong `Field` để có nhãn; đứng một mình thì phải truyền `aria-label`. */
export function TextInput({ type = "text", size = "md", ...rest }: TextInputProps) {
  const field = useFieldControl();
  const { t } = useTranslation();
  const [shown, setShown] = useState(false);
  const sizeClass = size === "lg" ? styles.lg : undefined;

  if (type === "password") {
    const label = t(shown ? "ui.hidePassword" : "ui.showPassword");
    return (
      <span className={styles.passwordWrap}>
        <input
          id={field?.id}
          aria-describedby={field?.describedBy}
          aria-invalid={field?.invalid || undefined}
          {...rest}
          type={shown ? "text" : "password"}
          className={cx(styles.control, styles.password, sizeClass)}
        />
        <button
          type="button"
          className={styles.passwordToggle}
          aria-label={label}
          title={label}
          disabled={rest.disabled}
          onClick={() => setShown((current) => !current)}
        >
          {shown ? <EyeOff size={16} aria-hidden /> : <Eye size={16} aria-hidden />}
        </button>
      </span>
    );
  }

  return (
    <input
      id={field?.id}
      aria-describedby={field?.describedBy}
      aria-invalid={field?.invalid || undefined}
      {...rest}
      type={type}
      className={cx(styles.control, sizeClass)}
    />
  );
}

interface SearchInputProps {
  value: string;
  onChange: (value: string) => void;
  /** Đã dịch; dùng làm cả placeholder và nhãn cho trình đọc màn hình. */
  label: string;
  autoFocus?: boolean;
}

/** Ô tìm kiếm có kính lúp, dùng chung cho mọi danh sách. */
export function SearchInput({ value, onChange, label, autoFocus }: SearchInputProps) {
  return (
    <span className={styles.searchWrap}>
      <Search className={styles.searchIcon} size={16} aria-hidden />
      <input
        type="search"
        className={cx(styles.control, styles.search)}
        value={value}
        placeholder={label}
        aria-label={label}
        data-autofocus={autoFocus || undefined}
        onChange={(e) => onChange(e.target.value)}
      />
    </span>
  );
}

type SelectProps = Omit<SelectHTMLAttributes<HTMLSelectElement>, "className" | "style" | "multiple">;

export function Select({ children, ...rest }: SelectProps) {
  const field = useFieldControl();
  return (
    <span className={styles.selectWrap}>
      <select
        id={field?.id}
        aria-describedby={field?.describedBy}
        aria-invalid={field?.invalid || undefined}
        {...rest}
        className={cx(styles.control, styles.select)}
      >
        {children}
      </select>
      <ChevronDown className={styles.chevron} size={16} aria-hidden />
    </span>
  );
}

type TextAreaProps = Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "className" | "style"> & {
  /** `code`: font mono, không tự xuống dòng, cao gần hết màn hình — dùng cho trình sửa file. */
  variant?: "default" | "code";
};

/** Ô nhập nhiều dòng. Đặt trong `Field` để có nhãn; đứng một mình thì phải truyền `aria-label`. */
export function TextArea({ variant = "default", ...rest }: TextAreaProps) {
  const field = useFieldControl();
  return (
    <textarea
      id={field?.id}
      aria-describedby={field?.describedBy}
      aria-invalid={field?.invalid || undefined}
      spellCheck={variant === "code" ? false : undefined}
      {...rest}
      className={cx(styles.control, styles.textarea, variant === "code" && styles.code)}
    />
  );
}
