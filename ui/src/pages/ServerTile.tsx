import { SquareTerminal } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Link, useNavigate } from "react-router";

import type { Server } from "../api/client";
import { playersApi } from "../api/players";
import { formatMemoryMb } from "../format";
import { AvatarStack, CopyField, Facts, IconButton, ServerCard, Stack, StatusBadge } from "../ui";
import { ServerActions } from "./server/ServerActions";
import { usePlayers } from "./server/players/usePlayers";
import { playAddress } from "./serverFilters";

export function serverName(server: Server): string {
  return server.name ?? server.id;
}

/** Thẻ một server: cách vào chơi, ai đang chơi, bật / tắt ngay trên thẻ (Trang chủ và trang Máy chủ). */
export function ServerTile({ server, lan }: { server: Server; lan: string[] | undefined }) {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const address = playAddress(server, lan);
  const ready = server.state !== "installing" && server.state !== "broken";
  const version = server.minecraft ? `${t(`loaders.${server.loader ?? "paper"}`)} ${server.minecraft}` : null;
  const memory = server.memory_mb ? formatMemoryMb(server.memory_mb.max, i18n.language) : null;
  const meta = version && memory ? t("tile.meta", { version, memory }) : version;

  return (
    <ServerCard
      seed={server.id}
      title={<Link to={`/servers/${server.id}`}>{serverName(server)}</Link>}
      status={<StatusBadge state={server.state} />}
      meta={meta}
      actions={
        ready && (
          <Stack direction="row" gap={2} justify="between" align="start">
            <IconButton icon={SquareTerminal} label={t("tile.openConsole")} onClick={() => void navigate(`/servers/${server.id}/console`)} />
            <ServerActions server={server} />
          </Stack>
        )
      }
    >
      {ready && (
        <Facts
          items={[
            { label: t("tile.address"), value: address ? <CopyField value={address} label={t("tile.copyAddress")} /> : "—" },
            { label: t("tile.playing"), value: <Playing server={server} /> },
          ]}
        />
      )}
    </ServerCard>
  );
}

function Playing({ server }: { server: Server }) {
  const { t } = useTranslation();
  const players = usePlayers(server.id, server.state);
  if (server.state === "starting") return <span>{t("tile.starting")}</span>;
  if (server.state !== "running") return <span>{t("tile.offline")}</span>;
  const data = players.data;
  if (!data) return <span>—</span>;
  const online = data.players.filter((p) => p.online);
  if (online.length === 0) return <span>{t("tile.nobody", { max: data.max_players })}</span>;
  return (
    <Stack direction="row" gap={2} align="center" wrap>
      <AvatarStack players={online.map((p) => ({ name: p.name, skinUrl: playersApi.skinUrl(p.uuid) }))} />
      <span>{t("tile.count", { count: online.length, max: data.max_players })}</span>
    </Stack>
  );
}
