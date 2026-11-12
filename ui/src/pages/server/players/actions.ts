import type { Player, PlayerActionKind } from "../../../api/players";

/** Vì sao một nút bị mờ; giao diện dịch thành câu giải thích. */
export type BlockReason = "stopped" | "offline" | "noTarget";

export interface ActionState {
  kind: PlayerActionKind;
  /** `null` là bấm được. */
  blocked: BlockReason | null;
}

export interface PlayerActionGroups {
  permissions: ActionState[];
  discipline: ActionState[];
  interact: ActionState[];
  message: ActionState[];
}

/**
 * Nút nào hiện và nút nào mờ (bảng ở spec P2, mục 6.7). Whitelist, op, cấm, bỏ cấm làm được cả khi
 * server tắt (MineHost sửa file). Thao tác còn lại cần server chạy và người đó đang trong game.
 * `others` là những người khác đang online, để dịch chuyển tới.
 */
export function playerActions(player: Player, running: boolean, others: string[]): PlayerActionGroups {
  const anytime = (kind: PlayerActionKind): ActionState => ({ kind, blocked: null });
  const live = (kind: PlayerActionKind, extra: BlockReason | null = null): ActionState => ({
    kind,
    blocked: !running ? "stopped" : !player.online ? "offline" : extra,
  });
  return {
    permissions: [anytime(player.whitelisted ? "whitelist_remove" : "whitelist_add"), anytime(player.op ? "deop" : "op")],
    discipline: [live("kick"), anytime(player.banned ? "pardon" : "ban")],
    interact: [
      live(player.frozen ? "unfreeze" : "freeze"),
      live("teleport_to_spawn"),
      live("teleport_to_player", others.length === 0 ? "noTarget" : null),
    ],
    message: [live("message"), live("title")],
  };
}

/** Những người khác đang online, để chọn đích dịch chuyển. */
export function teleportTargets(players: Player[], self: string): string[] {
  return players.filter((p) => p.online && p.name.toLowerCase() !== self.toLowerCase()).map((p) => p.name);
}
