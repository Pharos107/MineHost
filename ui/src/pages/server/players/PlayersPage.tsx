import { Users } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "../../../api/client";
import { playersApi, type Player } from "../../../api/players";
import { Avatar, Badge, Card, DataTable, EmptyState, Notice, Stack, type Column } from "../../../ui";
import { useServer } from "../ServerLayout";
import { AddPlayerCard } from "./AddPlayerCard";
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

  return (
    <Stack gap={5}>
      {data && !data.running && <Notice tone="info">{t("players.stoppedHint")}</Notice>}
      <AddPlayerCard serverId={server.id} players={data} />
      {query.isError ? (
        <Notice tone="danger">{errorMessage(t, query.error)}</Notice>
      ) : (
        <>
          <Card title={data ? t("players.onlineCount", { count: online.length, max: data.max_players }) : t("players.online")}>
            <PlayerTable label={t("players.online")} rows={online} loading={loading} empty={<p>{t("players.noneOnline")}</p>} />
          </Card>
          <Card title={t("players.known")}>
            <PlayerTable
              label={t("players.known")}
              rows={others}
              loading={loading}
              withLastSeen
              empty={<EmptyState icon={Users} title={t("players.noneKnown")} description={t("players.noneKnownHint")} />}
            />
          </Card>
        </>
      )}
    </Stack>
  );
}

interface PlayerTableProps {
  label: string;
  rows: Player[];
  loading: boolean;
  withLastSeen?: boolean;
  empty: ReactNode;
}

function PlayerTable({ label, rows, loading, withLastSeen = false, empty }: PlayerTableProps) {
  const columns = useColumns(withLastSeen);
  return (
    <DataTable
      label={label}
      columns={columns}
      rows={rows}
      rowKey={(p) => p.name.toLowerCase()}
      rowLabel={(p) => p.name}
      searchText={(p) => p.name}
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

/** Huy hiệu op / whitelist / bị cấm / đóng băng. Không có gì thì để trống. */
export function PlayerBadges({ player }: { player: Player }) {
  const { t } = useTranslation();
  return (
    <Stack direction="row" gap={1} wrap>
      {player.op && <Badge tone="primary">{t("players.badge.op")}</Badge>}
      {player.whitelisted && <Badge tone="success">{t("players.badge.whitelisted")}</Badge>}
      {player.banned && <Badge tone="danger">{t("players.badge.banned")}</Badge>}
      {player.frozen && <Badge tone="info">{t("players.badge.frozen")}</Badge>}
    </Stack>
  );
}
