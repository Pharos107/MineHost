import {
  ArrowRightLeft,
  Ban,
  Gavel,
  House,
  LogOut,
  Megaphone,
  MessageSquare,
  MousePointerClick,
  Send,
  ShieldCheck,
  ShieldOff,
  Snowflake,
  Sun,
  type LucideIcon,
} from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import type { Player, PlayerActionBody, PlayerActionKind } from "../../../api/players";
import { Button, DetailSection, Field, Select, Stack, Switch, TextInput, type ButtonVariant } from "../../../ui";
import type { ActionState } from "./actions";

/** Cách các nhóm gửi thao tác; `PlayerDetail` giữ một bản cho cả khung. */
export interface ActionRunner {
  run: (body: PlayerActionBody) => void;
  busy: boolean;
  pending: (kind: PlayerActionKind) => boolean;
  notice: string | null;
  error: string | null;
}

const ICONS: Partial<Record<PlayerActionKind, LucideIcon>> = {
  kick: LogOut,
  ban: Ban,
  pardon: ShieldOff,
  freeze: Snowflake,
  unfreeze: Sun,
  teleport_to_spawn: House,
  teleport_to_player: ArrowRightLeft,
  message: Send,
  title: Megaphone,
};

interface ActionButtonProps {
  state: ActionState;
  runner: ActionRunner;
  body?: Omit<PlayerActionBody, "action">;
  variant?: ButtonVariant;
  /** Thiếu dữ liệu để gửi (ví dụ chưa gõ nội dung). */
  incomplete?: boolean;
}

function ActionButton({ state, runner, body, variant = "secondary", incomplete = false }: ActionButtonProps) {
  const { t } = useTranslation();
  return (
    <Button
      size="sm"
      variant={variant}
      icon={ICONS[state.kind]}
      disabled={state.blocked !== null || incomplete || runner.busy}
      loading={runner.pending(state.kind)}
      onClick={() => runner.run({ action: state.kind, ...body })}
    >
      {t(`players.action.${state.kind}`)}
    </Button>
  );
}

/** Whitelist và op là công tắc: nhìn là biết đang bật hay tắt, làm được cả khi server tắt. */
export function PermissionsSection({ player, runner }: { player: Player; runner: ActionRunner }) {
  const { t } = useTranslation();
  return (
    <DetailSection title={t("players.group.permissions")} icon={ShieldCheck}>
      <Stack gap={3}>
        <Switch
          label={t("players.perm.whitelist")}
          description={t("players.perm.whitelistHint")}
          checked={player.whitelisted}
          disabled={runner.busy}
          onChange={(on) => runner.run({ action: on ? "whitelist_add" : "whitelist_remove" })}
        />
        <Switch
          label={t("players.perm.op")}
          description={t("players.perm.opHint")}
          checked={player.op}
          disabled={runner.busy}
          onChange={(on) => runner.run({ action: on ? "op" : "deop" })}
        />
      </Stack>
    </DetailSection>
  );
}

export function InteractSection({ states, targets, runner }: { states: ActionState[]; targets: string[]; runner: ActionRunner }) {
  const { t } = useTranslation();
  const [target, setTarget] = useState("");
  const chosen = targets.includes(target) ? target : (targets[0] ?? "");
  const [freeze, spawn, teleport] = states;
  return (
    <DetailSection title={t("players.group.interact")} icon={MousePointerClick}>
      <Stack direction="row" gap={2} wrap>
        {freeze && <ActionButton state={freeze} runner={runner} />}
        {spawn && <ActionButton state={spawn} runner={runner} />}
      </Stack>
      {teleport && (
        <Stack direction="row" gap={2} align="end" wrap>
          <Field label={t("players.target")}>
            <Select value={chosen} disabled={targets.length === 0} onChange={(e) => setTarget(e.target.value)}>
              {targets.length === 0 && <option value="">{t("players.noTarget")}</option>}
              {targets.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </Select>
          </Field>
          <ActionButton state={teleport} runner={runner} body={{ target: chosen }} />
        </Stack>
      )}
    </DetailSection>
  );
}

export function MessageSection({ states, runner }: { states: ActionState[]; runner: ActionRunner }) {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  return (
    <DetailSection title={t("players.group.message")} icon={MessageSquare}>
      <Field label={t("players.text")}>
        <TextInput value={text} maxLength={256} autoComplete="off" onChange={(e) => setText(e.target.value)} />
      </Field>
      <Stack direction="row" gap={2} wrap>
        {states.map((s) => (
          <ActionButton key={s.kind} state={s} runner={runner} body={{ text }} incomplete={text.trim() === ""} />
        ))}
      </Stack>
    </DetailSection>
  );
}

/** Đuổi, cấm để cuối và tô đỏ: khó hoàn tác, người chơi thấy lý do. */
export function DisciplineSection({ states, runner }: { states: ActionState[]; runner: ActionRunner }) {
  const { t } = useTranslation();
  const [reason, setReason] = useState("");
  return (
    <DetailSection title={t("players.group.discipline")} icon={Gavel} tone="danger">
      <Field label={t("players.reason")} hint={t("players.reasonHint")}>
        <TextInput value={reason} maxLength={200} autoComplete="off" onChange={(e) => setReason(e.target.value)} />
      </Field>
      <Stack direction="row" gap={2} wrap>
        {states.map((s) => (
          <ActionButton
            key={s.kind}
            state={s}
            runner={runner}
            body={s.kind === "pardon" ? undefined : { reason }}
            variant={s.kind === "ban" ? "danger" : "secondary"}
          />
        ))}
      </Stack>
    </DetailSection>
  );
}
