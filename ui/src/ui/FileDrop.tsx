import { Upload } from "lucide-react";
import { useRef, useState, type DragEvent, type ReactNode } from "react";

import { Button } from "./Button";
import { cx } from "./cx";
import styles from "./FileDrop.module.css";

export interface DroppedFile {
  file: File;
  path: string;
}

interface FileDropProps {
  /** Nhãn nút chọn file, ví dụ "Tải file lên". Biến thể `overlay` không có nút nên không dùng. */
  label: string;
  /** Dòng gợi ý trong vùng kéo thả; với `overlay` là chữ hiện trên lớp phủ khi đang kéo file. */
  hint: string;
  onFiles: (files: File[]) => void;
  /** Có thì đọc đệ quy cả thư mục kéo thả (`webkitGetAsEntry`) và trả đường dẫn tương đối. */
  onFilesWithPaths?: (files: DroppedFile[]) => void;
  /** Gọi khi kéo thả thư mục hoàn toàn rỗng (không chứa file nào). */
  onEmptyDrop?: () => void;
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

async function readEntry(entry: FileSystemEntry): Promise<DroppedFile[]> {
  if (entry.isFile) {
    const file = await new Promise<File>((resolve, reject) => (entry as FileSystemFileEntry).file(resolve, reject));
    const path = entry.fullPath.replace(/^\/+/, "") || file.name;
    return [{ file, path }];
  }
  if (!entry.isDirectory) return [];
  const reader = (entry as FileSystemDirectoryEntry).createReader();
  const out: DroppedFile[] = [];
  while (true) {
    const batch = await new Promise<FileSystemEntry[]>((resolve, reject) => reader.readEntries(resolve, reject));
    if (batch.length === 0) break;
    for (const child of batch) out.push(...(await readEntry(child)));
  }
  return out;
}

/** Vùng kéo thả file. Không tự tải lên: trang quyết định làm gì với file. */
export function FileDrop({ label, hint, onFiles, onFilesWithPaths, onEmptyDrop, accept, disabled = false, variant = "zone", children }: FileDropProps) {
  const input = useRef<HTMLInputElement>(null);
  const [over, setOver] = useState(false);
  // dragenter/dragleave bắn cả khi đi qua phần tử con: đếm để lớp phủ không nhấp nháy.
  const depth = useRef(0);

  function take(list: FileList | null) {
    const files = list ? Array.from(list) : [];
    if (files.length === 0 || disabled) return;
    if (onFilesWithPaths) onFilesWithPaths(files.map((file) => ({ file, path: file.webkitRelativePath || file.name })));
    else onFiles(files);
  }

  function handleDrop(e: DragEvent) {
    if (disabled) return;
    if (onFilesWithPaths && e.dataTransfer.items?.length) {
      // Lấy entry đồng bộ trước khi rời handler vì `DataTransfer` bị trình duyệt xóa sau đó.
      const entries = Array.from(e.dataTransfer.items)
        .map((item) => item.webkitGetAsEntry?.())
        .filter((entry): entry is FileSystemEntry => entry != null);
      if (entries.length > 0) {
        void Promise.all(entries.map(readEntry)).then((groups) => {
          const all = groups.flat();
          if (all.length > 0) onFilesWithPaths(all);
          else onEmptyDrop?.();
        });
        return;
      }
    }
    take(e.dataTransfer.files);
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
      handleDrop(e);
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
