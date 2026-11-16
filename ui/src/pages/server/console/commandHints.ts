/** Lệnh vanilla hay dùng; cách gõ và mô tả nằm ở `console.hints.<key>` trong i18n. */
export interface CommandHint {
  key: string;
  name: string;
}

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

const LIMIT = 6;

/**
 * Lệnh khớp chữ đang gõ. Chỉ gợi ý khi còn đang gõ tên lệnh (chưa có dấu cách); ô trống thì không,
 * để mũi tên lên / xuống vẫn là lịch sử lệnh.
 */
export function matchCommands(input: string): CommandHint[] {
  const typed = input.trimStart().replace(/^\//, "").toLowerCase();
  if (typed === "" || /\s/.test(typed)) return [];
  return COMMANDS.filter((c) => c.name.startsWith(typed)).slice(0, LIMIT);
}
