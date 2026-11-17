import type { ServerRegistry } from "../../../api/client";
import type { VanillaTuple } from "./vanillaItems";
import { VANILLA_REGISTRY } from "./vanillaRegistry";

export type RegistryKind = "effect" | "item" | "entity" | "enchantment" | "structure" | "biome";

export interface IndexedEntry {
  id: string;
  idLower: string;
  shortId: string;
  label: string;
  labelFolded: string;
  isMod: boolean;
}

export type RegistryIndex = Record<RegistryKind, IndexedEntry[]>;

/** Bỏ dấu tiếng Việt và viết thường một lần để tìm kiếm nhanh kể cả khi có hàng chục nghìn đồ mod. */
export function foldText(text: string): string {
  return text
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/đ/g, "d")
    .replace(/Đ/g, "d")
    .replace(/\s+/g, " ")
    .toLowerCase();
}

function buildBucket(
  vanilla: readonly VanillaTuple[],
  modEntries: readonly { id: string; label?: string | null }[] | undefined,
  lang: string,
): IndexedEntry[] {
  const useVi = !lang.toLowerCase().startsWith("en");
  const seen = new Set<string>();
  const out: IndexedEntry[] = [];

  for (const [shortId, vi, en] of vanilla) {
    const id = `minecraft:${shortId}`;
    seen.add(id);
    const label = useVi ? vi : en;
    // Ghép cả tên Việt và Anh vào `labelFolded` để gõ tiếng Anh lúc đang bật UI tiếng Việt vẫn tìm ra.
    out.push({
      id,
      idLower: id,
      shortId,
      label,
      labelFolded: foldText(`${vi} ${en}`),
      isMod: false,
    });
  }

  if (modEntries) {
    for (const entry of modEntries) {
      if (seen.has(entry.id)) continue;
      seen.add(entry.id);
      const idLower = entry.id.toLowerCase();
      const colon = idLower.indexOf(":");
      const shortId = colon >= 0 ? idLower.slice(colon + 1) : idLower;
      const label = entry.label ?? "";
      out.push({
        id: entry.id,
        idLower,
        shortId,
        label,
        labelFolded: foldText(label),
        isMod: true,
      });
    }
  }

  return out;
}

/** Dựng sẵn chỉ mục đã chuẩn hóa 1 lần cho toàn bộ vanilla + registry của server. */
export function buildRegistryIndex(serverRegistry?: ServerRegistry, lang = "vi"): RegistryIndex {
  return {
    effect: buildBucket(VANILLA_REGISTRY.effects, serverRegistry?.effects, lang),
    item: buildBucket(VANILLA_REGISTRY.items, serverRegistry?.items, lang),
    entity: buildBucket(VANILLA_REGISTRY.entities, serverRegistry?.entities, lang),
    enchantment: buildBucket(VANILLA_REGISTRY.enchantments, serverRegistry?.enchantments, lang),
    structure: buildBucket(VANILLA_REGISTRY.structures, undefined, lang),
    biome: buildBucket(VANILLA_REGISTRY.biomes, undefined, lang),
  };
}

/**
 * Tìm trong danh sách đã chuẩn hóa, ưu tiên khớp đầu ID rồi đầu từ mô tả, dừng sớm khi đủ `limit`
 * để gõ phím vẫn mượt ngay cả với 40.000+ vật phẩm của modpack lớn.
 */
export function searchRegistry(entries: readonly IndexedEntry[], query: string, limit = 24): IndexedEntry[] {
  const trimmed = query.trim();
  if (trimmed === "") return entries.slice(0, limit);

  const q = foldText(trimmed);
  const hasColon = q.includes(":");

  const tier1: IndexedEntry[] = [];
  const tier2: IndexedEntry[] = [];
  const tier3: IndexedEntry[] = [];

  for (let i = 0; i < entries.length; i += 1) {
    const entry = entries[i]!;
    if (entry.idLower.startsWith(q) || (!hasColon && entry.shortId.startsWith(q))) {
      tier1.push(entry);
      if (tier1.length >= limit) return tier1;
      continue;
    }
    if (entry.labelFolded.startsWith(q) || entry.labelFolded.includes(` ${q}`)) {
      if (tier2.length < limit) tier2.push(entry);
      continue;
    }
    if (tier1.length + tier2.length + tier3.length < limit && (entry.idLower.includes(q) || entry.labelFolded.includes(q))) {
      tier3.push(entry);
    }
  }

  return [...tier1, ...tier2, ...tier3].slice(0, limit);
}

const SHORT_ID_CACHE = new WeakMap<readonly IndexedEntry[], Set<string>>();

/** Kiểm tra O(1) xem token có khớp nguyên vẹn một `shortId` (`diamond`, `speed`…) không. */
export function hasExactShortId(entries: readonly IndexedEntry[], token: string): boolean {
  let set = SHORT_ID_CACHE.get(entries);
  if (!set) {
    set = new Set(entries.map((e) => e.shortId));
    SHORT_ID_CACHE.set(entries, set);
  }
  return set.has(token.toLowerCase());
}

export const EMPTY_REGISTRY_INDEX: RegistryIndex = buildRegistryIndex();
