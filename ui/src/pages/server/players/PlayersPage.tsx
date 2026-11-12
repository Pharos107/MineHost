import { UserRound, Users } from "lucide-react";
import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "../../../api/client";
import { playersApi, type Player } from "../../../api/players";
import { Avatar, Card, DataTable, EmptyState, Notice, Stack, type Column, type RowAction } from "../../../ui";
import { useServer } from "../ServerLayout";
import { AddPlayerCard } from "./AddPlayerCard";
import { PlayerBadges } from "./PlayerBadges";
import { PlayerDialog } from "./PlayerDialog";
import { usePlayers } from "./usePlayers";

/** Trang Người chơi (spec P2, mục 6.7): thêm theo tên, bảng Đang online và Đã từng vào. */
export function PlayersPage() {
  const { t } = useTranslation();
  const server = useServer();
  const query = usePlayers(server.id, server.state);
  const data = query.data;
  const online = data?.players.filter((p) => p.online) ?? [];
  const others = data?.players.filter((p) => !p.online) ?? [];
  const loading = query.isPending;
  const [picked, setPicked] = useState<Player | null>(null);
  const opened = picked && current(picked, data?.players ?? []);

  return (
    <Stack gap={5}>
      {data && !data.running && <Notice tone="info">{t("players.stoppedHint")}</Notice>}
      <AddPlayerCard serverId={server.id} players={data} />
      {query.isError ? (
        <Notice tone="danger">{errorMessage(t, query.error)}</Notice>
      ) : (
        <>
          <Card title={data ? t("players.onlineCount", { count: online.length, max: data.max_players }) : t("players.online")}>
            <PlayerTable label={t("players.online")} rows={online} loading={loading} onOpen={setPicked} empty={<p>{t("players.noneOnline")}</p>} />
          </Card>
          <Card title={t("players.known")}>
            <PlayerTable
              label={t("players.known")}
              rows={others}
              loading={loading}
              withLastSeen
              onOpen={setPicked}
              empty={<EmptyState icon={Users} title={t("players.noneKnown")} description={t("players.noneKnownHint")} />}
            />
          </Card>
        </>
      )}
      {data && opened && (
        <PlayerDialog serverId={server.id} player={opened} players={data.players} running={data.running} onClose={() => setPicked(null)} />
      )}
    </Stack>
  );
}

/**
 * Bản mới nhất của người đang mở, để huy hiệu đổi ngay khi dữ liệu mới tới qua WebSocket. Người đó
 * không còn trong file nào (ví dụ vừa bỏ cấm người chưa vào bao giờ) thì vẫn giữ hộp thoại, không còn
 * trạng thái nào.
 */
function current(picked: Player, players: Player[]): Player {
  const fresh = players.find((p) => p.name.toLowerCase() === picked.name.toLowerCase());
  return fresh ?? { ...picked, online: false, whitelisted: false, op: false, banned: false, ban_reason: null, frozen: false };
}

interface PlayerTableProps {
  label: string;
  rows: Player[];
  loading: boolean;
  withLastSeen?: boolean;
  onOpen: (player: Player) => void;
  empty: ReactNode;
}

function PlayerTable({ label, rows, loading, withLastSeen = false, onOpen, empty }: PlayerTableProps) {
  const { t } = useTranslation();
  const columns = useColumns(withLastSeen);
  const actions: RowAction<Player>[] = [{ label: t("players.details"), icon: UserRound, onSelect: onOpen }];
  return (
    <DataTable
      label={label}
      columns={columns}
      rows={rows}
      rowKey={(p) => p.name.toLowerCase()}
      rowLabel={(p) => p.name}
      searchText={(p) => p.name}
      actions={actions}
      inlineActions={1}
      loading={loading}
      empty={empty}
    />
  );
}

function useColumns(withLastSeen: boolean): Column<Player>[] {
  const { t, i18n } = useTranslation();
  const time = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });
  const columns: Column<Player>[] = [
    {
      id: "name",
      header: t("players.name"),
      cell: (p) => (
        <Stack direction="row" gap={2} align="center">
          <Avatar name={p.name} skinUrl={playersApi.skinUrl(p.uuid)} size="sm" />
          <span>{p.name}</span>
        </Stack>
      ),
      sortValue: (p) => p.name.toLowerCase(),
    },
    { id: "status", header: t("players.status"), cell: (p) => <PlayerBadges player={p} /> },
  ];
  if (withLastSeen) {
    columns.push({
      id: "lastSeen",
      header: t("players.lastSeen"),
      cell: (p) => (p.last_seen ? time.format(Date.parse(p.last_seen)) : t("players.neverJoined")),
      sortValue: (p) => p.last_seen ?? "",
      width: "md",
    });
  }
  return columns;
}
