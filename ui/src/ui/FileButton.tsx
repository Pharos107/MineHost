import { Upload, type LucideIcon } from "lucide-react";
import { useRef } from "react";

import { Button, type ButtonSize, type ButtonVariant } from "./Button";

interface FileButtonProps {
  label: string;
  onFiles: (files: File[]) => void;
  accept?: string;
  multiple?: boolean;
  disabled?: boolean;
  loading?: boolean;
  icon?: LucideIcon;
  variant?: ButtonVariant;
  size?: ButtonSize;
}

/** Nút mở hộp chọn file của trình duyệt. Dùng cùng `FileDrop variant="overlay"` khi không cần khung kéo thả. */
export function FileButton({
  label,
  onFiles,
  accept,
  multiple = true,
  disabled = false,
  loading = false,
  icon = Upload,
  variant = "secondary",
  size = "md",
}: FileButtonProps) {
  const input = useRef<HTMLInputElement>(null);
  return (
    <>
      <Button
        icon={icon}
        variant={variant}
        size={size}
        disabled={disabled}
        loading={loading}
        onClick={() => input.current?.click()}
      >
        {label}
      </Button>
      <input
        ref={input}
        type="file"
        accept={accept}
        multiple={multiple}
        hidden
        aria-label={label}
        onChange={(e) => {
          const files = e.target.files ? Array.from(e.target.files) : [];
          // Chọn lại đúng file đó lần nữa vẫn phải báo.
          e.target.value = "";
          if (files.length > 0) onFiles(files);
        }}
      />
    </>
  );
}
