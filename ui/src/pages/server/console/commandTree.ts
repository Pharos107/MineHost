import type { ServerCommands } from "../../../api/client";

export type ArgKind =
  | "subcommand"
  | "mode"
  | "player"
  | "rule"
  | "number"
  | "boolean"
  | "effect"
  | "item"
  | "entity"
  | "enchantment"
  | "structure"
  | "biome"
  | "text"
  | "other";

export interface TreeOption {
  value: string;
  /** Khóa i18n dưới `console.args.` để dịch mô tả nếu có. */
  detailKey?: string;
  next?: TreeNode;
}

export interface TreeNode {
  /** Nhãn hiển thị trên thanh cú pháp khi không dùng `paramKey`. */
  param: string;
  /** Khóa i18n dưới `console.params.` cho tham số chuẩn. */
  paramKey?: string;
  optional?: boolean;
  kind: ArgKind;
  options?: TreeOption[];
  next?: TreeNode;
}

type ScannedCommand = ServerCommands["commands"][number];

const NUMERIC_GAMERULES = new Set([
  "commandModificationBlockLimit",
  "maxCommandChainLength",
  "maxEntityCramming",
  "playersSleepingPercentage",
  "randomTickSpeed",
  "snowAccumulationHeight",
  "spawnRadius",
]);

export const VANILLA_GAMERULES = [
  "keepInventory",
  "doDaylightCycle",
  "doWeatherCycle",
  "mobGriefing",
  "doMobSpawning",
  "doFireTick",
  "naturalRegeneration",
  "randomTickSpeed",
  "showDeathMessages",
  "announceAdvancements",
  "doImmediateRespawn",
  "doInsomnia",
  "fallDamage",
  "fireDamage",
  "drowningDamage",
  "freezeDamage",
  "forgiveDeadPlayers",
  "universalAnger",
  "playersSleepingPercentage",
  "spawnRadius",
  "reducedDebugInfo",
  "commandBlockOutput",
  "sendCommandFeedback",
  "logAdminCommands",
  "doTileDrops",
  "doMobLoot",
  "doEntityDrops",
  "doPatrolSpawning",
  "doTraderSpawning",
  "doWardenSpawning",
  "doLimitedCrafting",
  "disableRaids",
  "disableElytraMovementCheck",
  "maxEntityCramming",
  "maxCommandChainLength",
  "waterSourceConversion",
  "lavaSourceConversion",
];

const BOOL_OPTIONS: TreeOption[] = [
  { value: "true", detailKey: "boolean.true" },
  { value: "false", detailKey: "boolean.false" },
];

function playerNode(optional = false, paramKey = "player", next?: TreeNode): TreeNode {
  return { param: optional ? `[${paramKey}]` : `<${paramKey}>`, paramKey: optional ? `${paramKey}Opt` : paramKey, optional, kind: "player", next };
}

function presets(paramKey: string, values: string[], optional = true, next?: TreeNode): TreeNode {
  return {
    param: `[${paramKey}]`,
    paramKey: optional ? `${paramKey}Opt` : paramKey,
    optional,
    kind: "number",
    options: values.map((value) => ({ value })),
    next,
  };
}

const effectGiveNode: TreeNode = playerNode(
  false,
  "player",
  {
    param: "<effect>",
    paramKey: "effect",
    kind: "effect",
    next: presets(
      "seconds",
      ["30", "60", "300", "infinite"],
      true,
      presets("amplifier", ["0", "1", "2", "4"], true, {
        param: "[hideParticles]",
        paramKey: "hideParticlesOpt",
        optional: true,
        kind: "boolean",
        options: BOOL_OPTIONS,
      }),
    ),
  },
);

const effectClearNode: TreeNode = playerNode(true, "player", { param: "[effect]", paramKey: "effectOpt", optional: true, kind: "effect" });

const xpAmountNode: TreeNode = {
  ...presets("amount", ["1", "5", "10", "30", "100"], false, {
    param: "[levels|points]",
    optional: true,
    kind: "mode",
    options: [
      { value: "levels", detailKey: "xp.levels" },
      { value: "points", detailKey: "xp.points" },
    ],
  }),
  param: "<amount>",
  paramKey: "amount",
  optional: false,
};

export const COMMAND_TREES: Record<string, TreeNode | null> = {
  list: null,
  seed: null,
  "save-all": { param: "[flush]", optional: true, kind: "subcommand", options: [{ value: "flush", detailKey: "saveAll.flush" }] },
  say: { param: "<message>", paramKey: "message", kind: "text" },
  msg: playerNode(false, "player", { param: "<message>", paramKey: "message", kind: "text" }),
  tp: playerNode(false, "player", {
    ...playerNode(false, "target"),
    options: [{ value: "~ ~ ~", detailKey: "coords" }],
  }),
  teleport: playerNode(false, "player", {
    ...playerNode(false, "target"),
    options: [{ value: "~ ~ ~", detailKey: "coords" }],
  }),
  give: playerNode(false, "player", {
    param: "<item>",
    paramKey: "item",
    kind: "item",
    next: presets("count", ["1", "16", "64"], true),
  }),
  clear: playerNode(true, "player", {
    param: "[item]",
    paramKey: "itemOpt",
    optional: true,
    kind: "item",
    next: presets("maxCount", ["1", "16", "64"], true),
  }),
  summon: {
    param: "<entity>",
    paramKey: "entity",
    kind: "entity",
    next: presets("pos", ["~ ~ ~"], true),
  },
  enchant: playerNode(false, "player", {
    param: "<enchantment>",
    paramKey: "enchantment",
    kind: "enchantment",
    next: presets("level", ["1", "2", "3", "4", "5"], true),
  }),
  gamemode: {
    param: "<survival|creative|adventure|spectator>",
    kind: "mode",
    options: [
      { value: "survival", detailKey: "gamemode.survival" },
      { value: "creative", detailKey: "gamemode.creative" },
      { value: "adventure", detailKey: "gamemode.adventure" },
      { value: "spectator", detailKey: "gamemode.spectator" },
    ],
    next: playerNode(true),
  },
  difficulty: {
    param: "<peaceful|easy|normal|hard>",
    kind: "mode",
    options: [
      { value: "peaceful", detailKey: "difficulty.peaceful" },
      { value: "easy", detailKey: "difficulty.easy" },
      { value: "normal", detailKey: "difficulty.normal" },
      { value: "hard", detailKey: "difficulty.hard" },
    ],
  },
  time: {
    param: "<set|add|query>",
    kind: "subcommand",
    options: [
      {
        value: "set",
        detailKey: "time.set",
        next: {
          param: "<day|night|noon|midnight>",
          kind: "mode",
          options: [
            { value: "day", detailKey: "time.day" },
            { value: "noon", detailKey: "time.noon" },
            { value: "night", detailKey: "time.night" },
            { value: "midnight", detailKey: "time.midnight" },
          ],
        },
      },
      { value: "add", detailKey: "time.add", next: { ...presets("time", ["1000", "6000", "12000"], false), param: "<time>", paramKey: "amount", optional: false } },
      {
        value: "query",
        detailKey: "time.query",
        next: {
          param: "<daytime|gametime|day>",
          kind: "mode",
          options: [
            { value: "daytime", detailKey: "time.daytime" },
            { value: "gametime", detailKey: "time.gametime" },
            { value: "day", detailKey: "time.dayCount" },
          ],
        },
      },
    ],
  },
  weather: {
    param: "<clear|rain|thunder>",
    kind: "mode",
    options: [
      { value: "clear", detailKey: "weather.clear" },
      { value: "rain", detailKey: "weather.rain" },
      { value: "thunder", detailKey: "weather.thunder" },
    ],
    next: presets("duration", ["300", "600", "1200"], true),
  },
  gamerule: {
    param: "<rule>",
    paramKey: "rule",
    kind: "rule",
    options: VANILLA_GAMERULES.map((value) => ({ value })),
    next: { param: "[value]", paramKey: "valueOpt", optional: true, kind: "boolean", options: BOOL_OPTIONS },
  },
  op: playerNode(false),
  deop: playerNode(false),
  pardon: playerNode(false),
  kick: playerNode(false, "player", { param: "[reason]", paramKey: "reasonOpt", optional: true, kind: "text" }),
  ban: playerNode(false, "player", { param: "[reason]", paramKey: "reasonOpt", optional: true, kind: "text" }),
  whitelist: {
    param: "<add|remove|list|on|off|reload>",
    kind: "subcommand",
    options: [
      { value: "add", detailKey: "whitelist.add", next: playerNode(false) },
      { value: "remove", detailKey: "whitelist.remove", next: playerNode(false) },
      { value: "list", detailKey: "whitelist.list" },
      { value: "on", detailKey: "whitelist.on" },
      { value: "off", detailKey: "whitelist.off" },
      { value: "reload", detailKey: "whitelist.reload" },
    ],
  },
  kill: playerNode(true, "target"),
  effect: {
    param: "<give|clear>",
    kind: "subcommand",
    options: [
      { value: "give", detailKey: "effect.give", next: effectGiveNode },
      { value: "clear", detailKey: "effect.clear", next: effectClearNode },
    ],
  },
  xp: {
    param: "<add|set|query>",
    kind: "subcommand",
    options: [
      { value: "add", detailKey: "xp.add", next: playerNode(false, "player", xpAmountNode) },
      { value: "set", detailKey: "xp.set", next: playerNode(false, "player", xpAmountNode) },
      {
        value: "query",
        detailKey: "xp.query",
        next: playerNode(false, "player", {
          param: "<levels|points>",
          kind: "mode",
          options: [
            { value: "levels", detailKey: "xp.levels" },
            { value: "points", detailKey: "xp.points" },
          ],
        }),
      },
    ],
  },
  spawnpoint: playerNode(true, "player", presets("pos", ["~ ~ ~"], true)),
  setworldspawn: presets("pos", ["~ ~ ~"], true),
  locate: {
    param: "<structure|biome|poi>",
    kind: "subcommand",
    options: [
      { value: "structure", detailKey: "locate.structure", next: { param: "<structure>", paramKey: "structure", kind: "structure" } },
      { value: "biome", detailKey: "locate.biome", next: { param: "<biome>", paramKey: "biome", kind: "biome" } },
      { value: "poi", detailKey: "locate.poi", next: { param: "<poi>", paramKey: "poi", kind: "other" } },
    ],
  },
};

/** Tách các token trong chuỗi `usage` nhưng giữ nguyên khối `(...)`, `<...>`, `[...]`. */
export function splitUsageTokens(rest: string): string[] {
  const tokens: string[] = [];
  let current = "";
  let depth = 0;
  for (const ch of rest.trim()) {
    if (ch === "(" || ch === "<" || ch === "[") depth += 1;
    else if ((ch === ")" || ch === ">" || ch === "]") && depth > 0) depth -= 1;
    if (/\s/.test(ch) && depth === 0) {
      if (current !== "") tokens.push(current);
      current = "";
      continue;
    }
    current += ch;
  }
  if (current !== "") tokens.push(current);
  return tokens;
}

/** Rút danh sách nhánh `(a|b|c)` từ output `help`; bỏ mảnh cuối nếu chuỗi bị cắt bởi `…`. */
export function parseChoiceGroup(token: string): string[] {
  if (!token.startsWith("(")) return [];
  const truncated = token.endsWith("…") && !token.includes(")");
  const inner = token.slice(1).replace(/\)$/, "").replace(/…$/, "");
  const parts = inner.split("|").map((p) => p.trim()).filter(Boolean);
  if (truncated && parts.length > 1) parts.pop();
  return parts.filter((p) => /^[a-zA-Z0-9_.:-]+$/.test(p));
}

function tokenToNode(token: string, next?: TreeNode): TreeNode {
  const choices = parseChoiceGroup(token);
  if (choices.length > 0) {
    const param = choices.length <= 4 ? `(${choices.join("|")})` : `(${choices.slice(0, 3).join("|")}|…)`;
    return { param, kind: "subcommand", options: choices.map((value) => ({ value })), next };
  }
  if (/^[a-zA-Z0-9_.:-]+$/.test(token)) {
    return { param: token, kind: "subcommand", options: [{ value: token }], next };
  }
  const optional = token.startsWith("[");
  if (/(player|target|victim)/i.test(token)) {
    return { param: token, optional, kind: "player", next };
  }
  if (/effect/i.test(token)) return { param: token, optional, kind: "effect", next };
  if (/item/i.test(token)) return { param: token, optional, kind: "item", next };
  if (/entity/i.test(token)) return { param: token, optional, kind: "entity", next };
  if (/enchant/i.test(token)) return { param: token, optional, kind: "enchantment", next };
  if (/biome/i.test(token)) return { param: token, optional, kind: "biome", next };
  if (/structure/i.test(token)) return { param: token, optional, kind: "structure", next };
  return { param: token, optional, kind: "other", next };
}

/** Dựng cây đối số từ `usage` đã quét của server; xử lý cả alias dạng `tp -> teleport`. */
export function buildScannedTree(name: string, scanned: ScannedCommand[], visited = new Set<string>()): TreeNode | null {
  const key = name.toLowerCase();
  if (visited.has(key)) return null;
  visited.add(key);

  const entry = scanned.find((c) => c.name.toLowerCase() === key);
  if (!entry?.usage) return null;

  const alias = entry.usage.match(/^[^\s]+\s*->\s*([^\s]+)$/);
  if (alias?.[1]) {
    const target = alias[1].toLowerCase();
    return COMMAND_TREES[target] ?? buildScannedTree(target, scanned, visited);
  }

  const firstSpace = entry.usage.search(/\s/);
  if (firstSpace < 0) return null;
  const tokens = splitUsageTokens(entry.usage.slice(firstSpace + 1));
  let head: TreeNode | undefined;
  for (let i = tokens.length - 1; i >= 0; i -= 1) {
    head = tokenToNode(tokens[i]!, head);
  }
  return head ?? null;
}

/** Gộp danh sách gamerule mặc định với gamerule quét từ server (nếu có). */
export function resolveCommandTree(name: string, scanned: ScannedCommand[]): TreeNode | null {
  const lower = name.toLowerCase();
  const exactScanned = scanned.find((c) => c.name.toLowerCase() === lower);

  if (lower === "gamerule") {
    const base = COMMAND_TREES.gamerule!;
    const scannedRules = exactScanned?.usage ? parseChoiceGroup(exactScanned.usage.slice(exactScanned.usage.indexOf(" ") + 1)) : [];
    const seen = new Set(VANILLA_GAMERULES);
    const merged = [...VANILLA_GAMERULES, ...scannedRules.filter((r) => !seen.has(r))];
    return {
      ...base,
      options: merged.map((value) => ({ value })),
    };
  }

  if (lower in COMMAND_TREES) return COMMAND_TREES[lower] ?? null;
  if (exactScanned) return buildScannedTree(lower, scanned);
  if (lower.startsWith("minecraft:")) {
    const shortName = lower.slice("minecraft:".length);
    if (shortName in COMMAND_TREES) return COMMAND_TREES[shortName] ?? null;
  }
  return null;
}

/** Với `gamerule <tên luật>`, chọn gợi ý giá trị boolean hoặc số theo luật đã biết. */
export function gameruleValueNode(ruleName: string): TreeNode {
  if (NUMERIC_GAMERULES.has(ruleName)) {
    return presets("value", ["0", "1", "3", "10", "100"], true);
  }
  return { param: "[value]", paramKey: "valueOpt", optional: true, kind: "boolean", options: BOOL_OPTIONS };
}
