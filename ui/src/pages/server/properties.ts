import type { Property } from "../../api/client";

export interface CommonProperty {
  key: string;
  kind: "text" | "number" | "boolean" | "select";
  /** Giá trị Minecraft dùng khi file chưa có khóa này. */
  fallback: string;
  options?: string[];
  min?: number;
  max?: number;
}

/** Các khóa hay sửa nhất, hiện thành form; mọi khóa khác sửa trong bảng "Nâng cao". */
export const COMMON_PROPERTIES: CommonProperty[] = [
  { key: "motd", kind: "text", fallback: "A Minecraft Server" },
  { key: "max-players", kind: "number", fallback: "20", min: 1, max: 100000 },
  { key: "gamemode", kind: "select", fallback: "survival", options: ["survival", "creative", "adventure", "spectator"] },
  { key: "difficulty", kind: "select", fallback: "easy", options: ["peaceful", "easy", "normal", "hard"] },
  { key: "view-distance", kind: "number", fallback: "10", min: 2, max: 32 },
  { key: "simulation-distance", kind: "number", fallback: "10", min: 3, max: 32 },
  { key: "spawn-protection", kind: "number", fallback: "16", min: 0, max: 1000 },
  { key: "pvp", kind: "boolean", fallback: "true" },
  { key: "white-list", kind: "boolean", fallback: "false" },
  { key: "online-mode", kind: "boolean", fallback: "true" },
  { key: "allow-flight", kind: "boolean", fallback: "false" },
  { key: "enable-command-block", kind: "boolean", fallback: "false" },
];

/** MineHost ghi đè khóa này từ `instance.toml` trước mỗi lần bật; đổi cổng ở tab Cài đặt server. */
export const MANAGED_KEYS = new Set(["server-port"]);

const COMMON_KEYS = new Set(COMMON_PROPERTIES.map((p) => p.key));

/** Các khóa không hiện lại trong bảng Nâng cao (đã nằm ở phần trên hoặc do MineHost quản lý). */
export const HIDDEN_FROM_ADVANCED = new Set([...MANAGED_KEYS, ...COMMON_KEYS]);

export function getProperty(list: Property[], key: string): string | undefined {
  return list.find((p) => p.key === key)?.value;
}

/** Đổi giá trị tại chỗ để giữ thứ tự trong file; khóa chưa có thì thêm vào cuối. */
export function setProperty(list: Property[], key: string, value: string): Property[] {
  if (!list.some((p) => p.key === key)) return [...list, { key, value }];
  return list.map((p) => (p.key === key ? { key, value } : p));
}

export function removeProperty(list: Property[], key: string): Property[] {
  return list.filter((p) => p.key !== key);
}

export function sameProperties(a: Property[], b: Property[]): boolean {
  return a.length === b.length && a.every((p, i) => p.key === b[i]?.key && p.value === b[i]?.value);
}

/** Đếm số khóa khác giữa bản nháp và bản gốc (sửa giá trị hoặc mới thêm). */
export function countChangedProperties(draft: Property[], initial: Property[]): number {
  const initialMap = new Map(initial.map((p) => [p.key, p.value]));
  let count = 0;
  for (const prop of draft) {
    if (initialMap.get(prop.key) !== prop.value) count += 1;
  }
  for (const prop of initial) {
    if (!draft.some((p) => p.key === prop.key)) count += 1;
  }
  return count;
}
