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

interface Loaded {
  url: string;
  hat: boolean;
}

/** Đầu skin Minecraft của người chơi (spec P2, mục 6.12). */
export function Avatar({ name, skinUrl, size = "md" }: AvatarProps) {
  // Nhớ URL đã tải được, để đổi người (đổi URL) thì quay về chữ cái cho tới khi ảnh mới tải xong.
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const ready = Boolean(skinUrl) && loaded?.url === skinUrl;
  const skin = { "--skin": `url("${skinUrl ?? ""}")` } as CSSProperties;

  return (
    <span className={cx(styles.avatar, styles[size])} role="img" aria-label={name}>
      {skinUrl && !ready && (
        <img className={styles.probe} src={skinUrl} alt="" onLoad={(e) => setLoaded({ url: skinUrl, hat: showsHat(e.currentTarget) })} />
      )}
      {ready ? (
        <>
          <span className={styles.face} style={skin} aria-hidden />
          {loaded?.hat && <span className={cx(styles.face, styles.hat)} style={skin} aria-hidden />}
        </>
      ) : (
        <span aria-hidden>{name.slice(0, 1).toUpperCase()}</span>
      )}
    </span>
  );
}

/**
 * Có vẽ lớp mũ không. Skin kiểu cũ (64×32) hay tô kín vùng mũ, và Minecraft bỏ qua lớp mũ không có
 * điểm trong suốt nào; vẽ theo thì mặt bị che thành ô màu. Không đọc được điểm ảnh thì bỏ lớp mũ.
 */
function showsHat(img: HTMLImageElement): boolean {
  if (img.naturalWidth === 0) return false;
  const legacy = img.naturalHeight * 2 === img.naturalWidth;
  if (!legacy) return true;
  try {
    const canvas = document.createElement("canvas");
    canvas.width = 8;
    canvas.height = 8;
    const ctx = canvas.getContext("2d");
    if (!ctx) return false;
    const unit = img.naturalWidth / 64;
    ctx.drawImage(img, 40 * unit, 8 * unit, 8 * unit, 8 * unit, 0, 0, 8, 8);
    const alpha = ctx.getImageData(0, 0, 8, 8).data.filter((_, i) => i % 4 === 3);
    return alpha.some((a) => a < 255);
  } catch {
    return false;
  }
}
