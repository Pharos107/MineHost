import { useMutation, useQueryClient } from "@tanstack/react-query";
import { lazy, Suspense, useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "../../../api/client";
import { playersApi, playersKey, type Player, type PlayerActionBody, type PlayerActionKind } from "../../../api/players";
import { Avatar, Badge, CopyField, DetailHeader, Facts, Notice, Stack } from "../../../ui";
import { playerActions, teleportTargets } from "./actions";
import { relativeTime } from "./filters";
import { PlayerBadges } from "./PlayerBadges";
import { DisciplineSection, InteractSection, MessageSection, PermissionsSection, type ActionRunner } from "./PlayerSections";

// Thư viện 3D nặng: chỉ tải khi mở chi tiết một người (kế hoạch PR-19).
const SkinViewer = lazy(() => import("./SkinViewer"));

interface PlayerDetailProps {
  serverId: string;
  player: Player;
  /** Mọi người chơi của server, để chọn đích dịch chuyển. */
  players: Player[];
  running: boolean;
}

/** Khung chi tiết một người chơi: skin 3D, UUID, các nhóm thao tác (spec làm mới trang Người chơi). */
export function PlayerDetail({ serverId, player, players, running }: PlayerDetailProps) {
  const { t } = useTranslation();
  const runner = useActionRunner(serverId, player.name);
  const targets = teleportTargets(players, player.name);
  const groups = playerActions(player, running, targets);
  const hint = !running ? t("players.blocked.stopped") : !player.online ? t("players.blocked.offline") : null;

  return (
    <Stack gap={4}>
      <DetailHeader title={player.name} media={<Skin player={player} />} subtitle={<Seen player={player} />}>
        <Stack direction="row" gap={1} wrap>
          {player.online && (
            <Badge tone="success" dot>
              {t("players.playingNow")}
            </Badge>
          )}
          <PlayerBadges player={player} />
        </Stack>
        <Facts
          items={[
            { label: t("players.uuid"), value: player.uuid ? <CopyField value={player.uuid} label={t("players.copyUuid")} /> : t("players.uuidUnknown") },
            ...(player.ban_reason ? [{ label: t("players.banReason"), value: player.ban_reason }] : []),
          ]}
        />
      </DetailHeader>
      {hint && <Notice tone="info">{hint}</Notice>}
      {runner.error && <Notice tone="danger">{runner.error}</Notice>}
      {runner.notice && <Notice tone="success">{runner.notice}</Notice>}
      <PermissionsSection player={player} runner={runner} />
      <InteractSection states={groups.interact} targets={targets} runner={runner} />
      <MessageSection states={groups.message} runner={runner} />
      <DisciplineSection states={groups.discipline} runner={runner} />
    </Stack>
  );
}

/** Gửi một thao tác cho người đang mở; nhớ thao tác đang chờ để nút đó hiện vòng quay. */
function useActionRunner(serverId: string, name: string): ActionRunner {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [notice, setNotice] = useState<string | null>(null);
  const act = useMutation({
    mutationFn: (body: PlayerActionBody) => playersApi.act(serverId, name, body),
    onSuccess: (result) => {
      setNotice(t(result.done === "sent" ? "players.done.sent" : "players.done.saved"));
      void queryClient.invalidateQueries({ queryKey: playersKey(serverId) });
    },
  });
  return {
    run: (body: PlayerActionBody) => {
      setNotice(null);
      act.mutate(body);
    },
    busy: act.isPending,
    pending: (kind: PlayerActionKind) => act.isPending && act.variables.action === kind,
    notice,
    error: act.isError ? errorMessage(t, act.error) : null,
  };
}

/** "Lần cuối vào 2 giờ trước" kèm giờ cụ thể khi rê chuột. */
function Seen({ player }: { player: Player }) {
  const { t, i18n } = useTranslation();
  if (player.online) return null;
  if (!player.last_seen) return <>{t("players.neverJoined")}</>;
  const exact = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" }).format(Date.parse(player.last_seen));
  const when = relativeTime(player.last_seen, i18n.language) ?? exact;
  return <span title={exact}>{t("players.lastSeenAgo", { when })}</span>;
}

/** Skin 3D; chưa tải xong, không có UUID hoặc không có WebGL thì hiện đầu skin phẳng cỡ lớn. */
function Skin({ player }: { player: Player }) {
  const { t } = useTranslation();
  const [failed, setFailed] = useState(false);
  const onError = useCallback(() => setFailed(true), []);
  const url = playersApi.skinUrl(player.uuid);
  const flat = <Avatar name={player.name} skinUrl={failed ? null : url} size="xl" />;
  if (!url || failed) return flat;
  return (
    <Suspense fallback={flat}>
      <SkinViewer url={url} label={t("players.skinOf", { name: player.name })} onError={onError} />
    </Suspense>
  );
}
