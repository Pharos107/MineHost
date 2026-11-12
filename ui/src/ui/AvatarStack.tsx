import { useTranslation } from "react-i18next";

import { Avatar } from "./Avatar";
import styles from "./AvatarStack.module.css";

export interface StackedPlayer {
  name: string;
  skinUrl?: string | null;
}

interface AvatarStackProps {
  players: StackedPlayer[];
  /** Hiện tối đa ngần này đầu; còn lại gộp thành "+N". */
  max?: number;
}

/** Hàng đầu skin chồng lên nhau, ví dụ những người đang online trên thẻ server. */
export function AvatarStack({ players, max = 5 }: AvatarStackProps) {
  const { t } = useTranslation();
  const shown = players.slice(0, max);
  const rest = players.length - shown.length;
  return (
    <span className={styles.stack} role="group" aria-label={players.map((p) => p.name).join(", ")}>
      {shown.map((p) => (
        <span key={p.name} className={styles.item} title={p.name}>
          <Avatar name={p.name} skinUrl={p.skinUrl} size="sm" />
        </span>
      ))}
      {rest > 0 && (
        <span className={styles.more} aria-label={t("ui.morePlayers", { count: rest })}>
          +{rest}
        </span>
      )}
    </span>
  );
}
