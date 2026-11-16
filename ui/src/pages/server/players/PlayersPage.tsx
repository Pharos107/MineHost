import { Ban, Crown, ListChecks, UserRound, Users } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "../../../api/client";
import type { Player, Players } from "../../../api/players";
import { EmptyState, Notice, SplitView, Stack, StatGrid, StatTile } from "../../../ui";
import { useServer } from "../ServerLayout";
import { AddPlayerDialog } from "./AddPlayerDialog";
import { countPlayers } from "./filters";
import { PlayerDetail } from "./PlayerDetail";
import { PlayerList } from "./PlayerList";
import { usePlayers } from "./usePlayers";

/**
 * Trang Người chơi (spec làm mới trang Người chơi): bốn ô số, danh sách gộp bên trái, chi tiết người
 * đang chọn bên phải. Màn hình hẹp thì chi tiết thay chỗ danh sách.
 */
export function PlayersPage() {
  const { t } = useTranslation();
  const server = useServer();
  const query = usePlayers(server.id, server.state);
  const data = query.data;
  const [picked, setPicked] = useState<Player | null>(null);
  const [adding, setAdding] = useState(false);
  const opened = picked && current(picked, data?.players ?? []);

  return (
    <Stack gap={5}>
      {data && !data.running && <Notice tone="info">{t("players.stoppedHint")}</Notice>}
      {query.isError ? (
        <Notice tone="danger">{errorMessage(t, query.error)}</Notice>
      ) : (
        <>
          <Summary data={data} />
          <SplitView
            detailOpen={opened !== null}
            backLabel={t("players.backToList")}
            onBack={() => setPicked(null)}
            list={
              <PlayerList
                players={data?.players ?? []}
                loading={query.isPending}
                selected={opened?.name ?? null}
                onSelect={setPicked}
                onAdd={() => setAdding(true)}
              />
            }
            detail={
              data && opened ? (
                <PlayerDetail key={opened.name.toLowerCase()} serverId={server.id} player={opened} players={data.players} running={data.running} />
              ) : (
                <EmptyState icon={UserRound} title={t("players.pickHint")} description={t("players.pickHintDetail")} />
              )
            }
          />
        </>
      )}
      <AddPlayerDialog open={adding} onClose={() => setAdding(false)} serverId={server.id} whitelistEnabled={data?.whitelist_enabled ?? true} />
    </Stack>
  );
}

/** Đang chơi, whitelist, op, bị cấm: nhìn qua là biết server đang mở cho ai. */
function Summary({ data }: { data: Players | undefined }) {
  const { t } = useTranslation();
  const counts = data ? countPlayers(data.players) : null;
  const value = (n: number | undefined) => (n === undefined ? "—" : String(n));
  return (
    <StatGrid>
      <StatTile icon={Users} label={t("players.stat.online")} value={value(counts?.online)} unit={data ? `/ ${data.max_players}` : undefined} />
      <StatTile
        icon={ListChecks}
        label={t("players.stat.whitelist")}
        value={value(counts?.whitelisted)}
        detail={data ? t(data.whitelist_enabled ? "players.stat.whitelistOn" : "players.stat.whitelistOff") : undefined}
      />
      <StatTile icon={Crown} label={t("players.stat.op")} value={value(counts?.op)} />
      <StatTile icon={Ban} label={t("players.stat.banned")} value={value(counts?.banned)} />
    </StatGrid>
  );
}

/**
 * Bản mới nhất của người đang mở, để huy hiệu đổi ngay khi dữ liệu mới tới qua WebSocket. Người đó
 * không còn trong file nào (ví dụ vừa bỏ cấm người chưa vào bao giờ) thì vẫn giữ khung chi tiết, không
 * còn trạng thái nào.
 */
function current(picked: Player, players: Player[]): Player {
  const fresh = players.find((p) => p.name.toLowerCase() === picked.name.toLowerCase());
  return fresh ?? { ...picked, online: false, whitelisted: false, op: false, banned: false, ban_reason: null, frozen: false };
}
