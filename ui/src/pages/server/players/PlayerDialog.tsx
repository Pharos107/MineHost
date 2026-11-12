import { useMutation, useQueryClient } from "@tanstack/react-query";
import { lazy, Suspense, useCallback, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "../../../api/client";
import { playersApi, playersKey, type Player, type PlayerActionBody, type PlayerActionKind } from "../../../api/players";
import { Avatar, Button, CopyField, Dialog, DialogBody, Facts, Field, Notice, Select, Stack, TextInput } from "../../../ui";
import { playerActions, teleportTargets, type ActionState } from "./actions";
import { PlayerBadges } from "./PlayerBadges";

// Thư viện 3D nặng: chỉ tải khi mở hộp thoại (kế hoạch PR-19).
const SkinViewer = lazy(() => import("./SkinViewer"));

interface PlayerDialogProps {
  serverId: string;
  player: Player;
  /** Mọi người chơi của server, để chọn đích dịch chuyển. */
  players: Player[];
  running: boolean;
  onClose: () => void;
}

/** Chi tiết một người chơi: skin 3D, UUID, các nhóm thao tác (spec P2, mục 6.7). */
export function PlayerDialog({ serverId, player, players, running, onClose }: PlayerDialogProps) {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const [reason, setReason] = useState("");
  const [text, setText] = useState("");
  const targets = teleportTargets(players, player.name);
  const [target, setTarget] = useState("");
  const chosenTarget = targets.includes(target) ? target : (targets[0] ?? "");
  const [notice, setNotice] = useState<string | null>(null);
  const act = useMutation({
    mutationFn: (body: PlayerActionBody) => playersApi.act(serverId, player.name, body),
    onSuccess: (result) => {
      setNotice(t(result.done === "sent" ? "players.done.sent" : "players.done.saved"));
      void queryClient.invalidateQueries({ queryKey: playersKey(serverId) });
    },
  });
  const run = (kind: PlayerActionKind) => {
    setNotice(null);
    const body: PlayerActionBody = { action: kind };
    if (kind === "kick" || kind === "ban") body.reason = reason;
    if (kind === "message" || kind === "title") body.text = text;
    if (kind === "teleport_to_player") body.target = chosenTarget;
    act.mutate(body);
  };
  const groups = playerActions(player, running, targets);
  const button = (state: ActionState, disabled = false) => (
    <Button
      key={state.kind}
      size="sm"
      variant={state.kind === "ban" || state.kind === "kick" ? "danger" : "secondary"}
      disabled={state.blocked !== null || disabled || act.isPending}
      loading={act.isPending && act.variables.action === state.kind}
      onClick={() => run(state.kind)}
    >
      {t(`players.action.${state.kind}`)}
    </Button>
  );
  const time = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });
  const hint = !running ? t("players.blocked.stopped") : !player.online ? t("players.blocked.offline") : null;

  return (
    <Dialog open onClose={onClose} title={player.name}>
      <DialogBody>
        <Stack gap={4}>
          <Stack direction="row" gap={4} align="center" wrap>
            <Skin player={player} />
            <Facts
              items={[
                { label: t("players.uuid"), value: player.uuid ? <CopyField value={player.uuid} label={t("players.copyUuid")} /> : t("players.uuidUnknown") },
                { label: t("players.lastSeen"), value: player.last_seen ? time.format(Date.parse(player.last_seen)) : t("players.neverJoined") },
                { label: t("players.status"), value: <PlayerBadges player={player} /> },
                ...(player.ban_reason ? [{ label: t("players.banReason"), value: player.ban_reason }] : []),
              ]}
            />
          </Stack>
          {hint && <Notice tone="info">{hint}</Notice>}
          {act.isError && <Notice tone="danger">{errorMessage(t, act.error)}</Notice>}
          {notice && <Notice tone="success">{notice}</Notice>}
          <Facts
            items={[
              { label: t("players.group.permissions"), value: <ButtonRow>{groups.permissions.map((s) => button(s))}</ButtonRow> },
              {
                label: t("players.group.discipline"),
                value: (
                  <Stack gap={2}>
                    <Field label={t("players.reason")} hint={t("players.reasonHint")}>
                      <TextInput value={reason} maxLength={200} autoComplete="off" onChange={(e) => setReason(e.target.value)} />
                    </Field>
                    <ButtonRow>{groups.discipline.map((s) => button(s))}</ButtonRow>
                  </Stack>
                ),
              },
              {
                label: t("players.group.interact"),
                value: (
                  <Stack gap={2}>
                    <ButtonRow>{groups.interact.slice(0, 2).map((s) => button(s))}</ButtonRow>
                    <Stack direction="row" gap={2} align="end" wrap>
                      <Field label={t("players.target")}>
                        <Select value={chosenTarget} disabled={targets.length === 0} onChange={(e) => setTarget(e.target.value)}>
                          {targets.length === 0 && <option value="">{t("players.noTarget")}</option>}
                          {targets.map((name) => (
                            <option key={name} value={name}>
                              {name}
                            </option>
                          ))}
                        </Select>
                      </Field>
                      {groups.interact.slice(2).map((s) => button(s))}
                    </Stack>
                  </Stack>
                ),
              },
              {
                label: t("players.group.message"),
                value: (
                  <Stack gap={2}>
                    <Field label={t("players.text")}>
                      <TextInput value={text} maxLength={256} autoComplete="off" onChange={(e) => setText(e.target.value)} />
                    </Field>
                    <ButtonRow>{groups.message.map((s) => button(s, text.trim() === ""))}</ButtonRow>
                  </Stack>
                ),
              },
            ]}
          />
        </Stack>
      </DialogBody>
    </Dialog>
  );
}

function ButtonRow({ children }: { children: ReactNode }) {
  return (
    <Stack direction="row" gap={2} wrap>
      {children}
    </Stack>
  );
}

/** Skin 3D; chưa tải xong, không có UUID hoặc không có WebGL thì hiện đầu skin phẳng. */
function Skin({ player }: { player: Player }) {
  const { t } = useTranslation();
  const [failed, setFailed] = useState(false);
  const onError = useCallback(() => setFailed(true), []);
  const url = playersApi.skinUrl(player.uuid);
  const flat = <Avatar name={player.name} skinUrl={failed ? null : url} size="lg" />;
  if (!url || failed) return flat;
  return (
    <Suspense fallback={flat}>
      <SkinViewer url={url} label={t("players.skinOf", { name: player.name })} onError={onError} />
    </Suspense>
  );
}
