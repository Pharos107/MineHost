import type { Server } from "../../api/client";

export type PropertyCategory = "world" | "performance" | "network" | "admin";
export type AdvancedFilter = "all" | PropertyCategory | "modified";
export type PropertyInputKind = "boolean" | "number" | "select" | "password" | "text";

export interface PropertyMeta {
  kind: PropertyInputKind;
  category: PropertyCategory;
  defaultValue?: string;
  options?: readonly string[];
  min?: number;
  max?: number;
  /** Chỉ có tác dụng khi tạo world mới hoặc đổi tên world. */
  newWorldOnly?: boolean;
}

export const ADVANCED_FILTERS: readonly AdvancedFilter[] = ["all", "world", "performance", "network", "admin", "modified"];

const LEVEL_TYPES = [
  "minecraft:normal",
  "minecraft:flat",
  "minecraft:large_biomes",
  "minecraft:amplified",
  "minecraft:single_biome_surface",
] as const;

const REGION_COMPRESSIONS = ["deflate", "lz4", "none"] as const;

/** Metadata cho các khóa chuẩn trong `server.properties` để chọn ô nhập và nhóm lọc phù hợp. */
export const PROPERTY_META: Record<string, PropertyMeta> = {
  // Thế giới & Gameplay
  "level-name": { kind: "text", category: "world", defaultValue: "world", newWorldOnly: true },
  "level-seed": { kind: "text", category: "world", defaultValue: "", newWorldOnly: true },
  "level-type": { kind: "select", category: "world", defaultValue: "minecraft:normal", options: LEVEL_TYPES, newWorldOnly: true },
  "generator-settings": { kind: "text", category: "world", defaultValue: "{}", newWorldOnly: true },
  "generate-structures": { kind: "boolean", category: "world", defaultValue: "true", newWorldOnly: true },
  "allow-nether": { kind: "boolean", category: "world", defaultValue: "true" },
  hardcore: { kind: "boolean", category: "world", defaultValue: "false" },
  "force-gamemode": { kind: "boolean", category: "world", defaultValue: "false" },
  "spawn-monsters": { kind: "boolean", category: "world", defaultValue: "true" },
  "spawn-animals": { kind: "boolean", category: "world", defaultValue: "true" },
  "spawn-npcs": { kind: "boolean", category: "world", defaultValue: "true" },
  "max-world-size": { kind: "number", category: "world", defaultValue: "29999984", min: 1, max: 29999984 },
  "max-build-height": { kind: "number", category: "world", defaultValue: "256", min: 64, max: 512 },

  // Hiệu năng
  "entity-broadcast-range-percentage": { kind: "number", category: "performance", defaultValue: "100", min: 10, max: 1000 },
  "max-tick-time": { kind: "number", category: "performance", defaultValue: "60000", min: -1 },
  "max-chained-neighbor-updates": { kind: "number", category: "performance", defaultValue: "1000000" },
  "sync-chunk-writes": { kind: "boolean", category: "performance", defaultValue: "true" },
  "use-native-transport": { kind: "boolean", category: "performance", defaultValue: "true" },
  "region-file-compression": { kind: "select", category: "performance", defaultValue: "deflate", options: REGION_COMPRESSIONS },
  "pause-when-empty-seconds": { kind: "number", category: "performance", defaultValue: "60", min: -1 },

  // Mạng & Bảo mật
  "enforce-secure-profile": { kind: "boolean", category: "network", defaultValue: "true" },
  "prevent-proxy-connections": { kind: "boolean", category: "network", defaultValue: "false" },
  "network-compression-threshold": { kind: "number", category: "network", defaultValue: "256", min: -1 },
  "rate-limit": { kind: "number", category: "network", defaultValue: "0", min: 0 },
  "server-ip": { kind: "text", category: "network", defaultValue: "" },
  "accepts-transfers": { kind: "boolean", category: "network", defaultValue: "false" },
  "log-ips": { kind: "boolean", category: "network", defaultValue: "true" },
  "require-resource-pack": { kind: "boolean", category: "network", defaultValue: "false" },
  "resource-pack": { kind: "text", category: "network", defaultValue: "" },
  "resource-pack-prompt": { kind: "text", category: "network", defaultValue: "" },
  "resource-pack-sha1": { kind: "text", category: "network", defaultValue: "" },
  "resource-pack-id": { kind: "text", category: "network", defaultValue: "" },
  "hide-online-players": { kind: "boolean", category: "network", defaultValue: "false" },
  "enable-status": { kind: "boolean", category: "network", defaultValue: "true" },
  "player-idle-timeout": { kind: "number", category: "network", defaultValue: "0", min: 0 },

  // Quản trị & Khác
  "enforce-whitelist": { kind: "boolean", category: "admin", defaultValue: "false" },
  "op-permission-level": { kind: "number", category: "admin", defaultValue: "4", min: 0, max: 4 },
  "function-permission-level": { kind: "number", category: "admin", defaultValue: "2", min: 1, max: 4 },
  "broadcast-console-to-ops": { kind: "boolean", category: "admin", defaultValue: "true" },
  "broadcast-rcon-to-ops": { kind: "boolean", category: "admin", defaultValue: "true" },
  "enable-rcon": { kind: "boolean", category: "admin", defaultValue: "false" },
  "rcon.port": { kind: "number", category: "admin", defaultValue: "25575", min: 1, max: 65535 },
  "rcon.password": { kind: "password", category: "admin", defaultValue: "" },
  "enable-query": { kind: "boolean", category: "admin", defaultValue: "false" },
  "query.port": { kind: "number", category: "admin", defaultValue: "25565", min: 1, max: 65535 },
  "enable-jmx-monitoring": { kind: "boolean", category: "admin", defaultValue: "false" },
  "enable-code-of-conduct": { kind: "boolean", category: "admin", defaultValue: "false" },
  debug: { kind: "boolean", category: "admin", defaultValue: "false" },
  "bug-report-link": { kind: "text", category: "admin", defaultValue: "" },
  "text-filtering-config": { kind: "text", category: "admin", defaultValue: "" },
  "initial-disabled-packs": { kind: "text", category: "admin", defaultValue: "" },
  "initial-enabled-packs": { kind: "text", category: "admin", defaultValue: "vanilla" },
};

export interface ResolvedPropertyControl {
  kind: PropertyInputKind;
  category: PropertyCategory;
  meta?: PropertyMeta;
}

/** Xác định kiểu control và nhóm cho một khóa (kể cả khóa lạ do mod/bản mới thêm). */
export function resolvePropertyControl(key: string, value: string): ResolvedPropertyControl {
  const meta = PROPERTY_META[key];
  const isBoolLiteral = value === "true" || value === "false";
  if (!meta) {
    return {
      kind: isBoolLiteral ? "boolean" : "text",
      category: "admin",
    };
  }
  if (meta.kind === "boolean") {
    return {
      kind: isBoolLiteral || value === "" ? "boolean" : "text",
      category: meta.category,
      meta,
    };
  }
  return { kind: meta.kind, category: meta.category, meta };
}

const MOD_LOADERS = new Set(["fabric", "quilt", "forge", "neoforge"]);

export function isModServer(server: Server): boolean {
  return (
    server.content_kind === "mods" ||
    server.modpack != null ||
    (server.loader !== undefined && server.loader !== null && MOD_LOADERS.has(server.loader))
  );
}

const PROPERTY_KEY_PATTERN = /^[a-zA-Z0-9._-]+$/;

export function isValidPropertyKey(key: string): boolean {
  const trimmed = key.trim();
  return trimmed.length > 0 && PROPERTY_KEY_PATTERN.test(trimmed);
}
