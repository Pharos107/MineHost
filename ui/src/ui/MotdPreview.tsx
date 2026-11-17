import { useState } from "react";

import { BlockBanner } from "./BlockBanner";
import { cx } from "./cx";
import { parseMotd, type MotdSpan } from "./motd";
import styles from "./MotdPreview.module.css";

interface MotdPreviewProps {
  /** Seed cho ảnh mặc định khi server chưa có `server-icon.png`. */
  serverId?: string;
  /** Tên server hiện ở dòng đầu của thẻ. */
  serverName?: string;
  /** Số người chơi tối đa (lấy từ `max-players`) để hiện `0/20`. */
  maxPlayers?: string;
  /** Đường dẫn tới `server-icon.png` nếu có. */
  iconUrl?: string;
  /** Nội dung `motd` có thể chứa mã `§`. */
  value: string;
  /** Chuỗi thay thế khi ô nhập đang trống. */
  fallback?: string;
  label?: string;
}

function spanDecoration(span: MotdSpan): string | undefined {
  if (span.underline && span.strikethrough) return "underline line-through";
  if (span.underline) return "underline";
  if (span.strikethrough) return "line-through";
  return undefined;
}

/** Xem trước thẻ server giống màn hình Multiplayer của Minecraft (icon, tên, số người chơi, ping, MOTD). */
export function MotdPreview({
  serverId = "MineHost",
  serverName = "Minecraft Server",
  maxPlayers = "20",
  iconUrl,
  value,
  fallback = "",
  label,
}: MotdPreviewProps) {
  const [brokenIconUrl, setBrokenIconUrl] = useState<string | null>(null);
  const lines = parseMotd(value || fallback);
  const showImage = Boolean(iconUrl && brokenIconUrl !== iconUrl);
  const maxLabel = maxPlayers.trim() || "20";

  return (
    <div className={styles.card} aria-label={label}>
      <div className={styles.iconBox} aria-hidden>
        {showImage ? (
          <img
            className={styles.iconImg}
            src={iconUrl}
            alt=""
            onError={() => setBrokenIconUrl(iconUrl ?? null)}
          />
        ) : (
          <BlockBanner seed={serverId} variant="logo" />
        )}
      </div>
      <div className={styles.body}>
        <div className={styles.topRow}>
          <span className={styles.serverName}>{serverName}</span>
          <span className={styles.meta} aria-hidden>
            <span>
              0<span className={styles.slash}>/</span>
              {maxLabel}
            </span>
            <span className={styles.ping}>
              <span className={cx(styles.bar, styles.b1)} />
              <span className={cx(styles.bar, styles.b2)} />
              <span className={cx(styles.bar, styles.b3)} />
              <span className={cx(styles.bar, styles.b4)} />
              <span className={cx(styles.bar, styles.b5)} />
            </span>
          </span>
        </div>
        <div className={styles.motdLines}>
          {lines.map((spans, lineIndex) => (
            <div key={lineIndex} className={styles.line}>
              {spans.map((span, spanIndex) => (
                <span
                  key={spanIndex}
                  className={cx(span.bold && styles.bold, span.italic && styles.italic, span.obfuscated && styles.obfuscated)}
                  style={{
                    color: span.color,
                    textDecoration: spanDecoration(span),
                  }}
                >
                  {span.text}
                </span>
              ))}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
