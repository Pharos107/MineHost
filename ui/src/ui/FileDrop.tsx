import { Upload } from "lucide-react";
import { useRef, useState, type DragEvent, type ReactNode } from "react";

import { Button } from "./Button";
import { cx } from "./cx";
import styles from "./FileDrop.module.css";

interface FileDropProps {
  /** Nhãn nút chọn file, ví dụ "Tải file lên". Biến thể `overlay` không có nút nên không dùng. */
  label: string;
  /** Dòng gợi ý trong vùng kéo thả; với `overlay` là chữ hiện trên lớp phủ khi đang kéo file. */
  hint: string;
  onFiles: (files: File[]) => void;
  accept?: string;
  disabled?: boolean;
  /**
   * `zone`: khung viền đứt kèm nút chọn file. `overlay`: không có khung, bọc nội dung (ví dụ một
   * bảng) và chỉ hiện lớp phủ khi đang kéo file vào; nút chọn file đặt riêng bằng `FileButton`.
   */
  variant?: "zone" | "overlay";
  /** `zone`: tiến độ hoặc thông báo hiện bên dưới. `overlay`: nội dung được bọc. */
  children?: ReactNode;
}

function hasFiles(e: DragEvent): boolean {
  return Array.from(e.dataTransfer.types).includes("Files");
}

/** Vùng kéo thả file. Không tự tải lên: trang quyết định làm gì với file. */
export function FileDrop({ label, hint, onFiles, accept, disabled = false, variant = "zone", children }: FileDropProps) {
  const input = useRef<HTMLInputElement>(null);
  const [over, setOver] = useState(false);
  // dragenter/dragleave bắn cả khi đi qua phần tử con: đếm để lớp phủ không nhấp nháy.
  const depth = useRef(0);

  function take(list: FileList | null) {
    const files = list ? Array.from(list) : [];
    if (files.length > 0 && !disabled) onFiles(files);
  }

  const handlers = {
    onDragEnter: (e: DragEvent) => {
      if (!hasFiles(e)) return;
      e.preventDefault();
      depth.current += 1;
      if (!disabled) setOver(true);
    },
    onDragOver: (e: DragEvent) => {
      if (!hasFiles(e)) return;
      e.preventDefault();
      if (!disabled) setOver(true);
    },
    onDragLeave: (e: DragEvent) => {
      if (!hasFiles(e)) return;
      depth.current = Math.max(0, depth.current - 1);
      if (depth.current === 0) setOver(false);
    },
    onDrop: (e: DragEvent) => {
      if (!hasFiles(e)) return;
      e.preventDefault();
      depth.current = 0;
      setOver(false);
      take(e.dataTransfer.files);
    },
  };

  if (variant === "overlay") {
    return (
      <div className={styles.overlayZone} {...handlers}>
        {children}
        {over && (
          <div className={styles.overlay} aria-hidden>
            <Upload size={20} />
            <span>{hint}</span>
          </div>
        )}
      </div>
    );
  }

  return (
    <div className={cx(styles.zone, over && styles.over, disabled && styles.disabled)} {...handlers}>
      <div className={styles.row}>
        <Button icon={Upload} size="sm" disabled={disabled} onClick={() => input.current?.click()}>
          {label}
        </Button>
        <span className={styles.hint}>{hint}</span>
      </div>
      <input
        ref={input}
        type="file"
        accept={accept}
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
