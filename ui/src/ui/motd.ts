export interface MotdSpan {
  text: string;
  color?: string;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strikethrough: boolean;
  obfuscated: boolean;
}

/** Bảng màu chuẩn §0–§f của Minecraft Java Edition trên nền danh sách server tối. */
const MC_COLORS: Record<string, string> = {
  "0": "#000000",
  "1": "#0000aa",
  "2": "#00aa00",
  "3": "#00aaaa",
  "4": "#aa0000",
  "5": "#aa00aa",
  "6": "#ffaa00",
  "7": "#aaaaaa",
  "8": "#555555",
  "9": "#5555ff",
  a: "#55ff55",
  b: "#55ffff",
  c: "#ff5555",
  d: "#ff55ff",
  e: "#ffff55",
  f: "#ffffff",
};

interface FormatState {
  color?: string;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strikethrough: boolean;
  obfuscated: boolean;
}

const EMPTY_STATE: FormatState = {
  bold: false,
  italic: false,
  underline: false,
  strikethrough: false,
  obfuscated: false,
};

function parseLine(line: string): MotdSpan[] {
  const spans: MotdSpan[] = [];
  let state: FormatState = { ...EMPTY_STATE };
  let buffer = "";

  const flush = () => {
    if (!buffer) return;
    spans.push({ text: buffer, ...state });
    buffer = "";
  };

  for (let i = 0; i < line.length; i += 1) {
    const ch = line[i];
    const next = line[i + 1];
    if (ch === "§" && next !== undefined) {
      const code = next.toLowerCase();
      if (code in MC_COLORS) {
        flush();
        // Trong Minecraft, đổi màu cũng xóa các kiểu chữ (l, o, n, m, k) trước đó.
        state = { ...EMPTY_STATE, color: MC_COLORS[code] };
        i += 1;
        continue;
      }
      if (code === "r") {
        flush();
        state = { ...EMPTY_STATE };
        i += 1;
        continue;
      }
      if (code === "l" || code === "o" || code === "n" || code === "m" || code === "k") {
        flush();
        if (code === "l") state = { ...state, bold: true };
        if (code === "o") state = { ...state, italic: true };
        if (code === "n") state = { ...state, underline: true };
        if (code === "m") state = { ...state, strikethrough: true };
        if (code === "k") state = { ...state, obfuscated: true };
        i += 1;
        continue;
      }
    }
    buffer += ch;
  }
  flush();
  return spans;
}

/** Tách chuỗi MOTD có mã màu `§` (hoặc `\u00A7`) thành tối đa 2 dòng để xem trước giống danh sách server Minecraft. */
export function parseMotd(raw: string): MotdSpan[][] {
  const normalized = raw.replace(/\\u00a7/gi, "§").replace(/\\n/g, "\n");
  return normalized.split("\n").slice(0, 2).map(parseLine);
}
