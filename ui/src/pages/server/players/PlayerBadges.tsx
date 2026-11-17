import { useTranslation } from "react-i18next";

import type { Player } from "../../../api/players";
import { Badge, Stack } from "../../../ui";

/**
 * Huy hiệu op / whitelist / bị cấm / đóng băng. Không có gì thì để trống. `compact` (hàng trong danh
 * sách) bỏ whitelist cho tên có chỗ; xem whitelist bằng dải lọc hoặc khung chi tiết.
 */
export function PlayerBadges({ player, compact = false }: { player: Player; compact?: boolean }) {
  const { t } = useTranslation();
  return (
    <Stack direction="row" gap={1} wrap>
      {player.op && <Badge tone="primary">{t("players.badge.op")}</Badge>}
      {player.whitelisted && !compact && <Badge tone="success">{t("players.badge.whitelisted")}</Badge>}
      {player.banned && <Badge tone="danger">{t("players.badge.banned")}</Badge>}
      {player.frozen && <Badge tone="info">{t("players.badge.frozen")}</Badge>}
    </Stack>
  );
}
