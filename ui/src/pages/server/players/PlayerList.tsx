import { UserPlus, Users } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { playersApi, type Player } from "../../../api/players";
import {
  Avatar,
  Button,
  EmptyState,
  SearchInput,
  SegmentedControl,
  SelectList,
  SelectListGroup,
  SelectListItem,
  Spinner,
} from "../../../ui";
import { filterPlayers, PLAYER_FILTERS, relativeTime, type PlayerFilter } from "./filters";
import { PlayerBadges } from "./PlayerBadges";

interface PlayerListProps {
  players: Player[];
  loading: boolean;
  /** Tên người đang mở trong khung chi tiết. */
  selected: string | null;
  onSelect: (player: Player) => void;
  onAdd: () => void;
}

/** Một danh sách gộp: người đang chơi ở trên, người khác ở dưới; tìm theo tên và lọc theo nhóm. */
export function PlayerList({ players, loading, selected, onSelect, onAdd }: PlayerListProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<PlayerFilter>("all");
  const rows = filterPlayers(players, filter, query);
  const online = rows.filter((p) => p.online);
  const offline = rows.filter((p) => !p.online);
  const item = (p: Player) => (
    <PlayerRow key={p.name.toLowerCase()} player={p} selected={selected?.toLowerCase() === p.name.toLowerCase()} onSelect={onSelect} />
  );

  return (
    <SelectList
      label={t("players.list")}
      search={<SearchInput label={t("players.search")} value={query} onChange={setQuery} />}
      actions={
        <Button variant="primary" icon={UserPlus} onClick={onAdd}>
          {t("players.addShort")}
        </Button>
      }
      filters={
        <SegmentedControl
          label={t("players.filter.label")}
          size="sm"
          value={filter}
          onChange={setFilter}
          options={PLAYER_FILTERS.map((value) => ({ value, label: t(`players.filter.${value}`) }))}
        />
      }
    >
      {loading ? (
        <Spinner label={t("ui.loading")} />
      ) : players.length === 0 ? (
        <EmptyState icon={Users} title={t("players.noneKnown")} description={t("players.noneKnownHint")} />
      ) : rows.length === 0 ? (
        <SelectListGroup title={t(`players.filter.${filter}`)} count={0} empty={t("players.noMatch")} />
      ) : (
        <>
          {(filter === "all" || online.length > 0) && (
            <SelectListGroup title={t("players.online")} count={online.length} empty={t("players.noneOnline")}>
              {online.map(item)}
            </SelectListGroup>
          )}
          {offline.length > 0 && (
            <SelectListGroup title={t("players.offline")} count={offline.length}>
              {offline.map(item)}
            </SelectListGroup>
          )}
        </>
      )}
    </SelectList>
  );
}

function PlayerRow({ player, selected, onSelect }: { player: Player; selected: boolean; onSelect: (player: Player) => void }) {
  const { t, i18n } = useTranslation();
  const seen = player.last_seen ? relativeTime(player.last_seen, i18n.language) : null;
  const subtitle = player.online ? t("players.playingNow") : seen ? t("players.seenAgo", { when: seen }) : t("players.neverJoined");
  return (
    <SelectListItem
      leading={<Avatar name={player.name} skinUrl={playersApi.skinUrl(player.uuid)} online={player.online} />}
      title={player.name}
      subtitle={subtitle}
      trailing={<PlayerBadges player={player} compact />}
      selected={selected}
      onSelect={() => onSelect(player)}
    />
  );
}
