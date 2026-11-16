import type { Player } from "../../../api/players";

export type PlayerFilter = "all" | "online" | "op" | "whitelisted" | "banned";

export const PLAYER_FILTERS: PlayerFilter[] = ["all", "online", "op", "whitelisted", "banned"];

function matches(player: Player, filter: PlayerFilter): boolean {
  switch (filter) {
    case "all":
      return true;
    case "online":
      return player.online;
    case "op":
      return player.op;
    case "whitelisted":
      return player.whitelisted;
    case "banned":
      return player.banned;
  }
}

/** Lọc theo nhóm và theo tên (không phân biệt hoa thường). Giữ thứ tự backend gửi. */
export function filterPlayers(players: Player[], filter: PlayerFilter, query: string): Player[] {
  const needle = query.trim().toLowerCase();
  return players.filter((p) => matches(p, filter) && (needle === "" || p.name.toLowerCase().includes(needle)));
}

export interface PlayerCounts {
  online: number;
  op: number;
  whitelisted: number;
  banned: number;
}

export function countPlayers(players: Player[]): PlayerCounts {
  return {
    online: players.filter((p) => p.online).length,
    op: players.filter((p) => p.op).length,
    whitelisted: players.filter((p) => p.whitelisted).length,
    banned: players.filter((p) => p.banned).length,
  };
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/**
 * "5 phút trước", "hôm qua"… cho lần cuối vào. Quá 30 ngày thì ghi ngày cụ thể, vì "3 tháng trước"
 * khó dùng khi cần biết chính xác. `null` khi chuỗi thời gian hỏng.
 */
export function relativeTime(iso: string, language: string, now: number = Date.now()): string | null {
  const at = Date.parse(iso);
  if (Number.isNaN(at)) return null;
  const diff = at - now;
  const ago = Math.abs(diff);
  const rtf = new Intl.RelativeTimeFormat(language, { numeric: "auto" });
  if (ago < MINUTE) return rtf.format(0, "minute");
  if (ago < HOUR) return rtf.format(Math.round(diff / MINUTE), "minute");
  if (ago < DAY) return rtf.format(Math.round(diff / HOUR), "hour");
  if (ago < 30 * DAY) return rtf.format(Math.round(diff / DAY), "day");
  return new Intl.DateTimeFormat(language, { dateStyle: "medium" }).format(at);
}
