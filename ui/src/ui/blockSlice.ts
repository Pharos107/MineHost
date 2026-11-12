/**
 * Lát cắt địa hình dạng khối, sinh từ một chuỗi (id server). Cùng chuỗi luôn ra cùng hình, để
 * mỗi server có "bộ mặt" riêng mà người dùng nhận ra ngay trong danh sách.
 */

interface Biome {
  /** Khối trên cùng của mỗi cột. */
  top: string[];
  /** Các lớp bên dưới. */
  soil: string[];
  /** Đốm lẻ trong lớp dưới (đá, quặng). */
  speck: string;
}

// Màu ở đây là hình minh họa, không phải màu giao diện, nên không lấy từ token.
const BIOMES: Biome[] = [
  { top: ["#6fae3f", "#7bbd47", "#64a03a"], soil: ["#7a5636", "#86603c", "#6d4c30"], speck: "#8f8f8a" },
  { top: ["#e3d39a", "#d9c88c", "#eadca8"], soil: ["#c9b277", "#bfa76d", "#d1bb82"], speck: "#a77f4b" },
  { top: ["#f1f4f6", "#e3e9ed", "#f7f9fa"], soil: ["#7a5636", "#86603c", "#6d4c30"], speck: "#9db4c6" },
  { top: ["#c96a3a", "#b85f34", "#d27a48"], soil: ["#a85a3a", "#c7835a", "#9b4f32"], speck: "#e0b25a" },
  { top: ["#8e2f2f", "#7a2828", "#9b3a33"], soil: ["#5b1f1f", "#6b2525", "#4f1b1b"], speck: "#f0a030" },
  { top: ["#3d78b8", "#4686c8", "#356da8"], soil: ["#d9c88c", "#c9b277", "#bfa76d"], speck: "#5aa0a0" },
  { top: ["#f0a8c8", "#e597ba", "#f5b9d4"], soil: ["#7a5636", "#86603c", "#6d4c30"], speck: "#c9e7a6" },
];

/** Biome cố định cho logo: đất có cỏ. */
export const PLAINS = 0;

/** FNV-1a 32 bit: đủ trộn cho một chuỗi ngắn, không cần an toàn mật mã. */
function hash(text: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

/** mulberry32: dãy số giả ngẫu nhiên lặp lại được từ một hạt giống. */
function random(seed: number): () => number {
  let a = seed;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Màu từng ô theo hàng, `null` là trời (để nền phía sau hiện ra). Mặt đất lên xuống từng bậc,
 * không bao giờ quá một phần ba chiều cao, để dải luôn đọc ra là địa hình. `flat`: mặt đất ở hàng
 * trên cùng, thành một khối đặc (biểu tượng nhỏ).
 */
export function blockSlice(seed: string, columns: number, rows: number, options: { biome?: number; flat?: boolean } = {}): (string | null)[][] {
  const { biome, flat = false } = options;
  const h = hash(seed);
  const next = random(h);
  const pick = (list: string[]) => list[Math.floor(next() * list.length)] ?? list[0] ?? null;
  const { top, soil, speck } = BIOMES[biome ?? h % BIOMES.length] ?? BIOMES[PLAINS]!;
  const lowest = flat ? 0 : Math.max(1, Math.floor(rows / 3));
  let surface = Math.floor(next() * (lowest + 1));
  const grid: (string | null)[][] = Array.from({ length: rows }, () => Array<string | null>(columns).fill(null));
  for (let x = 0; x < columns; x++) {
    const step = next();
    if (step < 0.25) surface = Math.max(0, surface - 1);
    else if (step > 0.75) surface = Math.min(lowest, surface + 1);
    for (let y = surface; y < rows; y++) {
      const row = grid[y]!;
      if (y === surface) row[x] = pick(top);
      else row[x] = next() < 0.06 ? speck : pick(soil);
    }
  }
  return grid;
}
