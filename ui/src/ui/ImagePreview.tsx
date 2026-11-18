import { useState } from "react";

import styles from "./ImagePreview.module.css";

interface ImagePreviewProps {
  src: string;
  alt: string;
  caption?: (dimensions: { width: number; height: number } | null) => string;
  onError?: () => void;
}

/** Khung xem trước ảnh (như `server-icon.png`), giữ nét pixel khi phóng và hiện kích thước thực. */
export function ImagePreview({ src, alt, caption, onError }: ImagePreviewProps) {
  const [dims, setDims] = useState<{ width: number; height: number } | null>(null);
  const text = caption?.(dims);
  return (
    <div className={styles.frame}>
      <img
        src={src}
        alt={alt}
        className={styles.image}
        onLoad={(e) => setDims({ width: e.currentTarget.naturalWidth, height: e.currentTarget.naturalHeight })}
        onError={onError}
      />
      {text && <div className={styles.caption}>{text}</div>}
    </div>
  );
}
