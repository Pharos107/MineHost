import { Upload } from "lucide-react";
import { useRef, useState, type DragEvent, type ReactNode } from "react";

import { Button } from "./Button";
import { cx } from "./cx";
import styles from "./FileDrop.module.css";

interface FileDropProps {
  /** Nhãn nút chọn file, ví dụ "Tải file lên". */
  label: string;
  /** Dòng gợi ý trong vùng kéo thả. */
  hint: string;
  onFiles: (files: File[]) => void;
  disabled?: boolean;
  /** Danh sách tiến độ hoặc thông báo hiện bên dưới. */
  children?: ReactNode;
}

/** Vùng kéo thả file kèm nút chọn file. Không tự tải lên: trang quyết định làm gì với file. */
export function FileDrop({ label, hint, onFiles, disabled = false, children }: FileDropProps) {
  const input = useRef<HTMLInputElement>(null);
  const [over, setOver] = useState(false);

  function take(list: FileList | null) {
    const files = list ? Array.from(list) : [];
    if (files.length > 0 && !disabled) onFiles(files);
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    setOver(false);
    take(e.dataTransfer.files);
  }

  return (
    <div
      className={cx(styles.zone, over && styles.over, disabled && styles.disabled)}
      onDragOver={(e) => {
        e.preventDefault();
        if (!disabled) setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={onDrop}
    >
      <div className={styles.row}>
        <Button icon={Upload} size="sm" disabled={disabled} onClick={() => input.current?.click()}>
          {label}
        </Button>
        <span className={styles.hint}>{hint}</span>
      </div>
      <input
        ref={input}
        type="file"
        multiple
        hidden
        aria-label={label}
        onChange={(e) => {
          take(e.target.files);
          // Chọn lại đúng file đó lần nữa vẫn phải báo.
          e.target.value = "";
        }}
      />
      {children}
    </div>
  );
}
