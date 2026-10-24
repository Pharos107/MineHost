import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import type { ServerState } from "../api/client";
import styles from "./Badge.module.css";
import { cx } from "./cx";

export type BadgeTone = "neutral" | "primary" | "info" | "success" | "warning" | "danger";

interface BadgeProps {
  tone?: BadgeTone;
  /** Chấm màu phía trước; `pulse` cho trạng thái đang chuyển. */
  dot?: boolean | "pulse";
  children: ReactNode;
}

export function Badge({ tone = "neutral", dot = false, children }: BadgeProps) {
  return (
    <span className={cx(styles.badge, styles[tone])}>
      {dot && <span className={cx(styles.dot, dot === "pulse" && styles.pulse)} aria-hidden />}
      {children}
    </span>
  );
}

const STATE_LOOK: Record<ServerState, { tone: BadgeTone; dot: BadgeProps["dot"] }> = {
  installing: { tone: "info", dot: "pulse" },
  stopped: { tone: "neutral", dot: true },
  starting: { tone: "info", dot: "pulse" },
  running: { tone: "success", dot: true },
  stopping: { tone: "warning", dot: "pulse" },
  crashed: { tone: "danger", dot: true },
  broken: { tone: "danger", dot: true },
};

/** Trạng thái server: màu và nhãn ở một chỗ để mọi trang hiển thị giống nhau. */
export function StatusBadge({ state }: { state: ServerState }) {
  const { t } = useTranslation();
  const look = STATE_LOOK[state];
  return (
    <Badge tone={look.tone} dot={look.dot}>
      {t(`serverState.${state}`)}
    </Badge>
  );
}
