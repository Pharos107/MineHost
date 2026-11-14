import type { LucideIcon } from "lucide-react";
import { useState, type CSSProperties } from "react";

import styles from "./Avatar.module.css";
import { cx } from "./cx";

interface AvatarProps {
  /** Tên người chơi hoặc mod; chữ cái đầu (hoặc icon) hiện khi chưa có ảnh, và là nhãn cho trình đọc màn hình. */
  name: string;
  /** Ảnh skin 64×64 (hoặc 64×32 kiểu cũ). Lấy phần mặt và lớp mũ. Không có hoặc tải lỗi thì hiện chữ. */
  skinUrl?: string | null;
  /** Ảnh thường (biểu tượng mod, plugin, dự án). Lỗi tải thì quay về `icon` hoặc chữ cái đầu. */
  imageUrl?: string | null;
  /** Biểu tượng thay thế khi không có ảnh. */
  icon?: LucideIcon;
  size?: "sm" | "md" | "lg";
}

interface Loaded {
  url: string;
  hat: boolean;
}

const ICON_SIZE = { sm: 14, md: 16, lg: 24 } as const;

/** Đầu skin Minecraft hoặc biểu tượng đại diện (spec P2, mục 6.12; spec P3, mục 6.3). */
export function Avatar({ name, skinUrl, imageUrl, icon: Icon, size = "md" }: AvatarProps) {
  // Nhớ URL đã tải được, để đổi người (đổi URL) thì quay về chữ cái cho tới khi ảnh mới tải xong.
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [failedImage, setFailedImage] = useState<string | null>(null);
  const ready = Boolean(skinUrl) && loaded?.url === skinUrl;
  const showImage = !skinUrl && Boolean(imageUrl) && failedImage !== imageUrl;
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
      ) : showImage && imageUrl ? (
        <img className={styles.image} src={imageUrl} alt="" onError={() => setFailedImage(imageUrl)} />
      ) : Icon ? (
        <Icon size={ICON_SIZE[size]} aria-hidden />
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
