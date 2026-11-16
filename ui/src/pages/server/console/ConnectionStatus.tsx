import { useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";

import { live, type LiveStatus } from "../../../api/ws";
import { Badge, type BadgeTone } from "../../../ui";

const TONE: Record<LiveStatus, BadgeTone> = { connecting: "info", open: "success", reconnecting: "warning" };

/** Trạng thái kết nối realtime, để khi mất mạng người dùng biết log đang đứng chứ không phải server im. */
export function ConnectionStatus() {
  const { t } = useTranslation();
  const status = useSyncExternalStore(
    (listener) => live.onStatus(listener),
    () => live.getStatus(),
  );
  return (
    <span role="status">
      <Badge tone={TONE[status]} dot={status === "open" ? true : "pulse"}>{t(`console.status.${status}`)}</Badge>
    </span>
  );
}
