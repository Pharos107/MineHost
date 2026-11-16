import type { ServerCommands } from "../../../api/client";

/** Lệnh vanilla hay dùng; cách gõ và mô tả nằm ở `console.hints.<key>` trong i18n. */
export interface CommandHint {
  key: string;
  name: string;
}

/** Một gợi ý: lệnh có sẵn (có `key` để dịch) hoặc lệnh quét được từ server. */
export interface MatchedCommand {
  name: string;
  key?: string;
  usage?: string | null;
  description?: string | null;
}

type ScannedCommand = ServerCommands["commands"][number];

export const COMMANDS: CommandHint[] = [
  { key: "list", name: "list" },
  { key: "say", name: "say" },
  { key: "msg", name: "msg" },
  { key: "tp", name: "tp" },
  { key: "give", name: "give" },
  { key: "gamemode", name: "gamemode" },
  { key: "difficulty", name: "difficulty" },
  { key: "time", name: "time" },
  { key: "weather", name: "weather" },
  { key: "gamerule", name: "gamerule" },
  { key: "op", name: "op" },
  { key: "deop", name: "deop" },
  { key: "kick", name: "kick" },
  { key: "ban", name: "ban" },
  { key: "pardon", name: "pardon" },
  { key: "whitelist", name: "whitelist" },
  { key: "saveAll", name: "save-all" },
  { key: "kill", name: "kill" },
  { key: "effect", name: "effect" },
  { key: "xp", name: "xp" },
  { key: "clear", name: "clear" },
  { key: "spawnpoint", name: "spawnpoint" },
  { key: "setworldspawn", name: "setworldspawn" },
  { key: "seed", name: "seed" },
  { key: "locate", name: "locate" },
];

const LIMIT = 8;
const BUILT_IN = new Set(COMMANDS.map((c) => c.name));

/** Khớp đầu tên, hoặc đầu phần sau dấu `:` (gõ `hour` ra `prominent:hourglass`). */
function startsWith(name: string, typed: string): boolean {
  const lower = name.toLowerCase();
  return lower.startsWith(typed) || (lower.split(":")[1]?.startsWith(typed) ?? false);
}

/**
 * Lệnh khớp chữ đang gõ: lệnh có sẵn trước, rồi lệnh của server chưa có trong danh sách sẵn. Chỉ gợi ý
 * khi còn đang gõ tên lệnh (chưa có dấu cách); ô trống thì không, để mũi tên lên / xuống vẫn là lịch sử lệnh.
 */
export function matchCommands(input: string, scanned: ScannedCommand[] = []): MatchedCommand[] {
  const typed = input.trimStart().replace(/^\//, "").toLowerCase();
  if (typed === "" || /\s/.test(typed)) return [];
  const own = COMMANDS.filter((c) => startsWith(c.name, typed));
  const extra = scanned.filter((c) => !BUILT_IN.has(c.name) && startsWith(c.name, typed));
  return [...own, ...extra].slice(0, LIMIT);
}
