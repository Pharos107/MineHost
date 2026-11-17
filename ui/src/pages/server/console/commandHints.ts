import type { ServerCommands } from "../../../api/client";
import { gameruleValueNode, resolveCommandTree, type ArgKind, type TreeNode, type TreeOption } from "./commandTree";
import {
  EMPTY_REGISTRY_INDEX,
  hasExactShortId,
  searchRegistry,
  type IndexedEntry,
  type RegistryIndex,
  type RegistryKind,
} from "./registrySearch";

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
  { key: "summon", name: "summon" },
  { key: "enchant", name: "enchant" },
];

const COMMAND_LIMIT = 8;
const ARG_LIMIT = 24;
const BUILT_IN = new Set(COMMANDS.map((c) => c.name));
const BUILT_IN_BY_NAME = new Map(COMMANDS.map((c) => [c.name, c]));

/** Khớp đầu tên, hoặc đầu phần sau dấu `:` / `.` (gõ `hour` ra `prominent:hourglass`). */
function startsWithSegment(name: string, typed: string): boolean {
  if (typed === "") return true;
  const lower = name.toLowerCase();
  if (lower.startsWith(typed)) return true;
  return lower.split(/[:.]/).some((part) => part.startsWith(typed));
}

/**
 * Lệnh khớp chữ đang gõ: lệnh có sẵn trước, rồi lệnh của server chưa có trong danh sách sẵn.
 * Ô trống hoàn toàn thì không gợi ý để mũi tên lên / xuống vẫn là lịch sử lệnh; gõ `/` thì mở danh sách.
 */
export function matchCommands(input: string, scanned: ScannedCommand[] = []): MatchedCommand[] {
  const trimmed = input.trimStart();
  if (trimmed === "") return [];
  const typed = trimmed.replace(/^\//, "").toLowerCase();
  if (/\s/.test(typed)) return [];
  if (typed === "" && !trimmed.startsWith("/")) return [];
  const own = COMMANDS.filter((c) => startsWithSegment(c.name, typed));
  const extra = scanned.filter((c) => !BUILT_IN.has(c.name) && startsWithSegment(c.name, typed));
  return [...own, ...extra].slice(0, COMMAND_LIMIT);
}

/** Gợi ý cho đối số đang gõ: tên người chơi hoặc bộ chọn `@a`, `@p`… */
export interface ArgumentHint {
  value: string;
  kind: "online" | "offline" | "selector";
}

/** Bộ chọn của Minecraft; mô tả ở `console.selector.<chữ>` trong i18n. */
export const SELECTORS = ["@a", "@p", "@r", "@s", "@e"];

function playerHints(token: string, players: { name: string; online: boolean }[], includeSelectorsWhenEmpty: boolean): ArgumentHint[] {
  const lower = token.toLowerCase();
  if (lower.startsWith("@")) {
    return SELECTORS.filter((s) => s.startsWith(lower)).map((value) => ({ value, kind: "selector" }));
  }
  const seen = new Set<string>();
  const hints: ArgumentHint[] = [];
  for (const player of [...players].sort((a, b) => Number(b.online) - Number(a.online))) {
    const key = player.name.toLowerCase();
    if (seen.has(key) || !key.startsWith(lower)) continue;
    seen.add(key);
    hints.push({ value: player.name, kind: player.online ? "online" : "offline" });
  }
  if (includeSelectorsWhenEmpty && lower === "") {
    for (const value of SELECTORS) hints.push({ value, kind: "selector" });
  }
  return hints;
}

/**
 * Hàm cũ giữ tương thích: đối số cuối đang gõ (sau dấu cách cuối). Chưa gõ chữ nào thì không gợi ý.
 */
export function matchArgument(input: string, players: { name: string; online: boolean }[]): { start: number; hints: ArgumentHint[] } {
  const start = Math.max(input.lastIndexOf(" "), input.lastIndexOf("\t")) + 1;
  const token = input.slice(start);
  if (start === 0 || token === "") return { start, hints: [] };
  return { start, hints: playerHints(token, players, false).slice(0, COMMAND_LIMIT) };
}

export interface TokenSpan {
  text: string;
  start: number;
  end: number;
}

/** Tách chữ đang gõ thành các token kèm vị trí chính xác để thay thế mà không làm mất `/` ở đầu. */
function tokenizeAfterPrefix(input: string, offset: number): TokenSpan[] {
  const spans: TokenSpan[] = [];
  let start = -1;
  for (let i = offset; i < input.length; i += 1) {
    if (/\s/.test(input[i]!)) {
      if (start >= 0) {
        spans.push({ text: input.slice(start, i), start, end: i });
        start = -1;
      }
    } else if (start < 0) {
      start = i;
    }
  }
  if (start >= 0) spans.push({ text: input.slice(start), start, end: input.length });
  return spans;
}

export type ConsoleHint =
  | { group: "command"; command: MatchedCommand }
  | { group: "option"; value: string; kind: ArgKind; detailKey?: string }
  | { group: "player"; value: string; kind: "online" | "offline" | "selector" }
  | { group: "registry"; kind: RegistryKind; id: string; label: string; isMod: boolean };

const REGISTRY_KINDS = new Set<ArgKind>(["effect", "item", "entity", "enchantment", "structure", "biome"]);

function isRegistryKind(kind: ArgKind): kind is RegistryKind {
  return REGISTRY_KINDS.has(kind);
}

export interface SyntaxNodeInfo {
  param: string;
  paramKey?: string;
  optional?: boolean;
  active: boolean;
}

export interface ConsoleSyntax {
  command: string;
  nodes: SyntaxNodeInfo[];
  descriptionKey?: string;
  descriptionText?: string;
}

export interface ConsoleAnalysis {
  stage: "idle" | "command" | "argument";
  start: number;
  hints: ConsoleHint[];
  syntax: ConsoleSyntax | null;
  fill: (value: string) => string;
}

function optionHints(node: TreeNode, token: string): ConsoleHint[] {
  const lower = token.toLowerCase();
  const options: TreeOption[] = node.options ?? [];
  return options
    .filter((opt) => startsWithSegment(opt.value, lower))
    .slice(0, ARG_LIMIT)
    .map((opt) => ({ group: "option", value: opt.value, kind: node.kind, detailKey: opt.detailKey }));
}

function isCompletedRegistryToken(entries: readonly IndexedEntry[], text: string, nextArg: string): boolean {
  if (text.includes(":") || text.includes("_")) return true;
  if (/^(\d|~|infinite$|true$|false$)/i.test(nextArg)) return true;
  if (hasExactShortId(entries, text)) return true;
  // Nếu đã gõ dấu cách sau từ không khớp mục nào trong registry, chuyển sang tham số kế tiếp.
  if (nextArg === "" && searchRegistry(entries, text, 1).length === 0) return true;
  return false;
}

function walkTree(
  cmdName: string,
  args: TokenSpan[],
  activeArgIndex: number,
  scanned: ScannedCommand[],
  registry: RegistryIndex,
): { activeNode: TreeNode | null; syntaxNodes: SyntaxNodeInfo[]; resolvedIndex: number } {
  const root = resolveCommandTree(cmdName, scanned);
  const syntaxNodes: SyntaxNodeInfo[] = [];
  let node: TreeNode | null = root;
  let resolvedIndex = activeArgIndex;

  for (let i = 0; i < activeArgIndex && node; i += 1) {
    const argText = args[i]?.text ?? "";
    if (isRegistryKind(node.kind) && !isCompletedRegistryToken(registry[node.kind], argText, args[i + 1]?.text ?? "")) {
      resolvedIndex = i;
      break;
    }
    syntaxNodes.push({ param: node.param, paramKey: node.paramKey, optional: node.optional, active: false });
    if (cmdName.toLowerCase() === "gamerule" && i === 0) {
      node = gameruleValueNode(argText);
      continue;
    }
    const matched = node.options?.find((opt) => opt.value.toLowerCase() === argText.toLowerCase());
    node = matched?.next ?? node.next ?? null;
  }

  const activeNode = node;
  let tail: TreeNode | null = node;
  let isCurrent = true;
  while (tail) {
    syntaxNodes.push({ param: tail.param, paramKey: tail.paramKey, optional: tail.optional, active: isCurrent });
    isCurrent = false;
    // Khi nhánh đang chọn đã khớp một lựa chọn cụ thể có cây con, hiện tiếp nhánh con của nó.
    if (tail === node && args[resolvedIndex]) {
      const argLower = args[resolvedIndex]!.text.toLowerCase();
      const matched = tail.options?.find((opt) => opt.value.toLowerCase() === argLower);
      tail = matched?.next ?? tail.next ?? null;
    } else {
      tail = tail.next ?? null;
    }
  }

  return { activeNode, syntaxNodes, resolvedIndex };
}

function commandDescription(cmdName: string, scanned: ScannedCommand[]): { descriptionKey?: string; descriptionText?: string } {
  const lower = cmdName.toLowerCase();
  const short = lower.startsWith("minecraft:") ? lower.slice("minecraft:".length) : lower;
  const builtIn = BUILT_IN_BY_NAME.get(lower) ?? BUILT_IN_BY_NAME.get(short);
  if (builtIn) return { descriptionKey: `console.hints.${builtIn.key}.desc` };
  const exact = scanned.find((c) => c.name.toLowerCase() === lower);
  if (exact?.description) return { descriptionText: exact.description };
  return {};
}

/** Phân tích toàn bộ ô lệnh hiện tại: trả về gợi ý ngữ cảnh, thanh cú pháp và hàm điền giữ nguyên `/`. */
export function analyzeConsole(
  input: string,
  scanned: ScannedCommand[] = [],
  players: { name: string; online: boolean }[] = [],
  registry: RegistryIndex = EMPTY_REGISTRY_INDEX,
): ConsoleAnalysis {
  const lead = input.match(/^(\s*\/?)/)?.[1] ?? "";
  const tokens = tokenizeAfterPrefix(input, lead.length);
  const trailingSpace = /\s$/.test(input);

  if (tokens.length === 0) {
    if (!lead.endsWith("/")) {
      return { stage: "idle", start: 0, hints: [], syntax: null, fill: (v) => `${v} ` };
    }
    const hints: ConsoleHint[] = matchCommands(input, scanned).map((command) => ({ group: "command", command }));
    return {
      stage: "command",
      start: lead.length,
      hints,
      syntax: null,
      fill: (v) => `${lead}${v} `,
    };
  }

  const cmdToken = tokens[0]!;
  if (tokens.length === 1 && !trailingSpace) {
    const hints: ConsoleHint[] = matchCommands(input, scanned).map((command) => ({ group: "command", command }));
    const lower = cmdToken.text.toLowerCase();
    const recognized = BUILT_IN_BY_NAME.has(lower) || scanned.some((c) => c.name.toLowerCase() === lower);
    const syntax = recognized
      ? {
          command: `${lead.trimStart()}${cmdToken.text}`,
          nodes: walkTree(cmdToken.text, [], 0, scanned, registry).syntaxNodes.map((n) => ({ ...n, active: false })),
          ...commandDescription(cmdToken.text, scanned),
        }
      : null;
    return {
      stage: "command",
      start: cmdToken.start,
      hints,
      syntax,
      fill: (v) => `${input.slice(0, cmdToken.start)}${v} `,
    };
  }

  const args = tokens.slice(1);
  const activeArgIndex = trailingSpace ? args.length : args.length - 1;
  const { activeNode, syntaxNodes, resolvedIndex } = walkTree(cmdToken.text, args, activeArgIndex, scanned, registry);
  const multiWord = resolvedIndex < activeArgIndex && args[resolvedIndex] !== undefined;
  const start = multiWord ? args[resolvedIndex]!.start : trailingSpace ? input.length : args[args.length - 1]!.start;
  const activeText = multiWord ? input.slice(start) : trailingSpace ? "" : args[args.length - 1]!.text;

  const hints: ConsoleHint[] = [];
  if (activeNode) {
    if (activeNode.kind === "player") {
      for (const p of playerHints(activeText, players, true).slice(0, ARG_LIMIT)) {
        hints.push({ group: "player", ...p });
      }
      if (activeNode.options) {
        for (const o of optionHints(activeNode, activeText)) hints.push(o);
      }
    } else if (isRegistryKind(activeNode.kind)) {
      for (const entry of searchRegistry(registry[activeNode.kind], activeText, ARG_LIMIT)) {
        hints.push({
          group: "registry",
          kind: activeNode.kind,
          id: entry.id,
          label: entry.label,
          isMod: entry.isMod,
        });
      }
    } else if (activeNode.kind !== "text") {
      hints.push(...optionHints(activeNode, activeText));
    }
  }

  const desc = commandDescription(cmdToken.text, scanned);
  const syntax: ConsoleSyntax | null =
    syntaxNodes.length > 0 || desc.descriptionKey || desc.descriptionText
      ? { command: `${lead.trimStart()}${cmdToken.text}`, nodes: syntaxNodes, ...desc }
      : null;

  return {
    stage: "argument",
    start,
    hints: hints.slice(0, ARG_LIMIT),
    syntax,
    fill: (v) => `${input.slice(0, start)}${v} `,
  };
}
