import { useState, type CSSProperties } from "react";

import styles from "./Avatar.module.css";
import { cx } from "./cx";

interface AvatarProps {
  /** Tên người chơi; chữ cái đầu hiện khi chưa có ảnh, và là nhãn cho trình đọc màn hình. */
  name: string;
  /** Ảnh skin 64×64 (hoặc 64×32 kiểu cũ). Lấy phần mặt và lớp mũ. Không có hoặc tải lỗi thì hiện chữ. */
  skinUrl?: string | null;
  size?: "sm" | "md" | "lg";
}

/** Đầu skin Minecraft của người chơi (spec P2, mục 6.12). */
export function Avatar({ name, skinUrl, size = "md" }: AvatarProps) {
  // Nhớ URL đã tải được, để đổi người (đổi URL) thì quay về chữ cái cho tới khi ảnh mới tải xong.
  const [loaded, setLoaded] = useState<string | null>(null);
  const ready = Boolean(skinUrl) && loaded === skinUrl;
  const face = { "--skin": `url("${skinUrl ?? ""}")` } as CSSProperties;

  return (
    <span className={cx(styles.avatar, styles[size])} role="img" aria-label={name}>
      {skinUrl && !ready && <img className={styles.probe} src={skinUrl} alt="" onLoad={() => setLoaded(skinUrl)} />}
      {ready ? <span className={styles.face} style={face} aria-hidden /> : <span aria-hidden>{name.slice(0, 1).toUpperCase()}</span>}
    </span>
  );
}
