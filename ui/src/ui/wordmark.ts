/**
 * Hình học logo chữ khối kiểu tiêu đề Minecraft: chữ pixel nét dày, mặt đá có texture nhiễu,
 * gờ sáng trên, vết nứt, khối 3D đổ xuống hội tụ về điểm tụ phía dưới giữa và viền đen dày.
 * Mọi hình xuất ra dạng chuỗi path SVG để cả logo chỉ cần vài chục phần tử DOM.
 */

export interface WordmarkCell {
  x: number;
  y: number;
  letterIndex: number;
  topEdge: boolean;
  bottomEdge: boolean;
  leftEdge: boolean;
  rightEdge: boolean;
}

export const WORDMARK_ROWS = 16;
/** Độ sâu khối 3D đo ở chân chữ (đơn vị ô). */
export const EXTRUDE_DEPTH = 5.5;
const EXTRUDE_LAYERS = 14;
/** Điểm tụ nằm dưới chân chữ bao xa; càng xa thì khối càng ít thu về giữa. */
const VANISH_BELOW = 64;

// Mỗi chữ cao 16 hàng, nét dày 4 ô như chữ MINECRAFT gốc. Chữ O khoét mặt Creeper ở giữa.
const GLYPHS: Record<string, string[]> = {
  M: [
    "####......####", "#####....#####", "######..######", "##############",
    "####.####.####", "####.####.####", "####..##..####", "####..##..####",
    "####......####", "####......####", "####......####", "####......####",
    "####......####", "####......####", "####......####", "####......####",
  ],
  I: Array.from({ length: 16 }, () => "#####"),
  N: [
    "####....####", "#####...####", "######..####", "#######.####",
    "############", "####.#######", "####..######", "####...#####",
    "####....####", "####....####", "####....####", "####....####",
    "####....####", "####....####", "####....####", "####....####",
  ],
  E: [
    "##########", "##########", "##########", "##########",
    "####......", "####......", "########..", "########..",
    "########..", "########..", "####......", "####......",
    "##########", "##########", "##########", "##########",
  ],
  H: [
    "####....####", "####....####", "####....####", "####....####",
    "####....####", "####....####", "############", "############",
    "############", "############", "####....####", "####....####",
    "####....####", "####....####", "####....####", "####....####",
  ],
  O: [
    "##############", "##############", "##############", "##############",
    "####..##..####", "####..##..####", "######..######", "#####....#####",
    "#####....#####", "#####.##.#####", "##############", "##############",
    "##############", "##############", "##############", "##############",
  ],
  S: [
    "###########", "###########", "###########", "###########",
    "####.......", "####.......", "###########", "###########",
    "###########", "###########", ".......####", ".......####",
    "###########", "###########", "###########", "###########",
  ],
  T: [
    "############", "############", "############", "############",
    "....####....", "....####....", "....####....", "....####....",
    "....####....", "....####....", "....####....", "....####....",
    "....####....", "....####....", "....####....", "....####....",
  ],
};

/** Vết nứt bậc thang trên mặt đá theo thứ tự chữ: `[dx, dy, dài, hướng]` tính theo ô của chữ. */
const LETTER_CRACKS: [number, number, number, "h" | "v"][][] = [
  [[0, 9, 2, "h"], [2, 9, 3, "v"], [10, 3, 2, "h"], [11, 1, 2, "v"]],
  [[0, 6, 2, "h"], [2, 6, 2, "v"], [2, 8, 2, "h"]],
  [[8, 10, 2, "h"], [9, 10, 3, "v"], [0, 1, 2, "h"]],
  [[5, 1, 2, "v"], [5, 3, 3, "h"], [4, 13, 2, "v"], [4, 13, 3, "h"]],
  [[0, 2, 2, "h"], [1, 2, 3, "v"], [8, 7, 3, "h"], [10, 12, 3, "v"]],
  [[1, 11, 3, "h"], [3, 11, 2, "v"], [10, 1, 2, "v"], [11, 2, 2, "h"]],
  [[7, 0, 2, "v"], [7, 2, 3, "h"], [1, 13, 2, "v"], [1, 15, 3, "h"]],
  [[9, 1, 2, "v"], [9, 3, 1, "h"], [5, 8, 3, "v"], [4, 11, 2, "h"]],
];

/** Ô của cả dòng chữ (chữ cách nhau 1 ô, từ cách nhau 3 ô) kèm cờ biên để vẽ gờ cạnh. */
export function wordmarkCells(words: string[]): { cells: WordmarkCell[]; columns: number; letterStarts: number[] } {
  const cells: WordmarkCell[] = [];
  const letterStarts: number[] = [];
  let x = 0;
  words.forEach((word, w) => {
    if (w > 0) x += 3;
    [...word.toUpperCase()].forEach((letter, i) => {
      const glyph = GLYPHS[letter];
      if (!glyph) return;
      if (i > 0) x += 1;
      const width = glyph[0]?.length ?? 0;
      const letterIndex = letterStarts.length;
      letterStarts.push(x);
      glyph.forEach((row, y) => {
        [...row].forEach((mark, dx) => {
          if (mark !== "#") return;
          cells.push({
            x: x + dx,
            y,
            letterIndex,
            topEdge: glyph[y - 1]?.[dx] !== "#",
            bottomEdge: glyph[y + 1]?.[dx] !== "#",
            leftEdge: row[dx - 1] !== "#",
            rightEdge: row[dx + 1] !== "#",
          });
        });
      });
      x += width;
    });
  });
  return { cells, columns: x, letterStarts };
}

function rect(x: number, y: number, w: number, h: number): string {
  const r = (n: number) => Number(n.toFixed(3));
  return `M${r(x)} ${r(y)}h${r(w)}v${r(h)}h${r(-w)}z`;
}

/** Gộp ô theo từng hàng thành dải ngang để path mặt chữ ngắn và không lộ đường nối giữa các ô. */
export function cellsPath(cells: { x: number; y: number }[], size = 1): string {
  const rows = new Map<number, number[]>();
  cells.forEach(({ x, y }) => rows.set(y, [...(rows.get(y) ?? []), x]));
  let d = "";
  rows.forEach((xs, y) => {
    xs.sort((a, b) => a - b);
    let start = xs[0] ?? 0;
    let prev = start;
    xs.slice(1).concat(Number.NaN).forEach((x) => {
      if (x === prev + 1) {
        prev = x;
        return;
      }
      d += rect(start * size, y * size, (prev - start + 1) * size, size);
      start = x;
      prev = x;
    });
  });
  return d;
}

type Edge = "topEdge" | "bottomEdge" | "leftEdge" | "rightEdge";

/** Dải gờ dày `t` dọc theo cạnh lộ ra của mặt chữ (gờ sáng phía trên, gờ tối phía dưới). */
export function edgePath(cells: WordmarkCell[], edge: Edge, t: number): string {
  return cells
    .filter((c) => c[edge])
    .map(({ x, y }) => {
      if (edge === "topEdge") return rect(x, y, 1, t);
      if (edge === "bottomEdge") return rect(x, y + 1 - t, 1, t);
      if (edge === "leftEdge") return rect(x, y, t, 1);
      return rect(x + 1 - t, y, t, 1);
    })
    .join("");
}

/** Hash số nguyên ổn định: texture giống nhau ở mọi lần vẽ và mọi máy. */
function hash(x: number, y: number, seed: number): number {
  let h = Math.imul(x, 374761393) ^ Math.imul(y, 668265263) ^ Math.imul(seed, 1442695041);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  h ^= h >>> 16;
  return (h >>> 0) / 4294967296;
}

/**
 * Ô texture đá `n × n` điểm ảnh, mỗi điểm cỡ `px`. Nhiễu mịn trộn với nhiễu khối 3×3 để có mảng
 * sẫm, mảng sáng như texture đá Minecraft. Bảng màu xếp từ tối tới sáng.
 */
export function noiseTile(n: number, px: number, palette: string[], seed: number): { fill: string; d: string }[] {
  const byColor = palette.map(() => [] as { x: number; y: number }[]);
  for (let y = 0; y < n; y += 1) {
    for (let x = 0; x < n; x += 1) {
      const v = 0.55 * hash(x, y, seed) + 0.45 * hash(Math.floor(x / 3), Math.floor(y / 3), seed + 7);
      const i = Math.min(palette.length - 1, Math.floor(v * palette.length));
      byColor[i]?.push({ x, y });
    }
  }
  return palette.map((fill, i) => ({ fill, d: cellsPath(byColor[i] ?? [], px) }));
}

/** Các lớp khối 3D từ xa tới gần: hệ số co về điểm tụ và độ tối phủ thêm (sọc từng ô như pixel). */
export function extrusionLayers(): { scale: number; shade: number }[] {
  const total = EXTRUDE_DEPTH / VANISH_BELOW;
  return Array.from({ length: EXTRUDE_LAYERS }, (_, i) => {
    const k = EXTRUDE_LAYERS - i;
    const band = Math.floor((k * EXTRUDE_DEPTH) / EXTRUDE_LAYERS);
    return { scale: 1 - (total * k) / EXTRUDE_LAYERS, shade: 0.028 * k + (band % 2 === 1 ? 0.07 : 0) };
  });
}

export function vanishingPoint(columns: number): [number, number] {
  return [columns / 2, WORDMARK_ROWS + VANISH_BELOW];
}

/** Vết nứt: nét tối và viền sáng ngay dưới/phải, chỉ giữ nét nằm trọn trong mặt chữ. */
export function crackPaths(cells: WordmarkCell[], letterStarts: number[]): { dark: string; light: string } {
  const filled = new Set(cells.map((c) => `${c.x},${c.y}`));
  let dark = "";
  let light = "";
  letterStarts.forEach((startX, idx) => {
    (LETTER_CRACKS[idx % LETTER_CRACKS.length] ?? []).forEach(([dx, dy, len, dir]) => {
      const x = startX + dx;
      const covered = Array.from({ length: len }, (_, i) =>
        dir === "h" ? `${x + i},${dy}` : `${x},${dy + i}`,
      );
      if (!covered.every((key) => filled.has(key))) return;
      if (dir === "h") {
        dark += rect(x, dy + 0.35, len, 0.4);
        light += rect(x, dy + 0.75, len, 0.2);
      } else {
        dark += rect(x + 0.35, dy, 0.4, len);
        light += rect(x + 0.75, dy, 0.2, len);
      }
    });
  });
  return { dark, light };
}
