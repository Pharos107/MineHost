import type { Property } from "../../api/client";

export interface CommonProperty {
  key: string;
  kind: "text" | "number" | "boolean" | "select";
  /** Giá trị Minecraft dùng khi file chưa có khóa này. */
  fallback: string;
  options?: string[];
}

/** Các khóa hay sửa nhất, hiện thành form; mọi khóa khác sửa trong bảng "Nâng cao". */
export const COMMON_PROPERTIES: CommonProperty[] = [
  { key: "motd", kind: "text", fallback: "A Minecraft Server" },
  { key: "max-players", kind: "number", fallback: "20" },
  { key: "gamemode", kind: "select", fallback: "survival", options: ["survival", "creative", "adventure", "spectator"] },
  { key: "difficulty", kind: "select", fallback: "easy", options: ["peaceful", "easy", "normal", "hard"] },
  { key: "pvp", kind: "boolean", fallback: "true" },
  { key: "white-list", kind: "boolean", fallback: "false" },
  { key: "online-mode", kind: "boolean", fallback: "true" },
];

/** MineHost ghi đè khóa này từ `instance.toml` trước mỗi lần bật; đổi cổng ở tab Cài đặt server. */
export const MANAGED_KEYS = new Set(["server-port"]);

export function getProperty(list: Property[], key: string): string | undefined {
  return list.find((p) => p.key === key)?.value;
}

/** Đổi giá trị tại chỗ để giữ thứ tự trong file; khóa chưa có thì thêm vào cuối. */
export function setProperty(list: Property[], key: string, value: string): Property[] {
  if (!list.some((p) => p.key === key)) return [...list, { key, value }];
  return list.map((p) => (p.key === key ? { key, value } : p));
}

export function sameProperties(a: Property[], b: Property[]): boolean {
  return a.length === b.length && a.every((p, i) => p.key === b[i]?.key && p.value === b[i]?.value);
}
