import type { TerminalTone } from "../../ui";

/** Một dòng đã nhận, giữ lại phía trình duyệt để lọc hoặc vẽ lại mà không phải hỏi server. */
export interface BufferedLine {
  text: string;
  tone: TerminalTone;
}

/** Bằng bộ đệm của supervisor và `scrollback` của xterm (spec P1, mục 5.6). */
export const MAX_LINES = 5000;

/** Thêm một dòng, bỏ dòng cũ nhất khi quá `max`. Sửa tại chỗ vì được gọi cho từng dòng log. */
export function appendCapped(lines: BufferedLine[], line: BufferedLine, max = MAX_LINES): void {
  lines.push(line);
  if (lines.length > max) lines.splice(0, lines.length - max);
}

// Mã màu ANSI của server (ví dụ `\x1b[33m`) không được tính khi lọc.
// eslint-disable-next-line no-control-regex
const ANSI = /\x1b\[[0-9;]*[A-Za-z]/g;

export type LineLevel = "info" | "warn" | "error";

// Nhãn mức chỉ đọc ở đầu dòng, để chat có chữ "WARN]" không bị tô màu. Paper: `[10:20:11 WARN]:`;
// vanilla, Fabric: `[10:20:11] [Server thread/WARN]:`; Forge: `[16Nov2026 10:20:11.123] [main/WARN] [mod/]:`.
const ERROR = /^(\[[^\]]*\]\s*)?\[[^\]]*?[\s/](ERROR|FATAL|SEVERE)\]/;
const WARN = /^(\[[^\]]*\]\s*)?\[[^\]]*?[\s/]WARN(ING)?\]/;

/** Mức của một dòng log, đọc từ nhãn trong ngoặc vuông. */
export function lineLevel(text: string): LineLevel {
  const plain = text.replace(ANSI, "");
  if (ERROR.test(plain)) return "error";
  if (WARN.test(plain)) return "warn";
  return "info";
}

/** Màu của một dòng: lệnh người gõ màu nhấn, stderr và `ERROR` đỏ, `WARN` vàng. */
export function lineTone(stream: "stdout" | "stderr" | "stdin", text: string): TerminalTone {
  if (stream === "stdin") return "primary";
  if (stream === "stderr") return "danger";
  const level = lineLevel(text);
  return level === "error" ? "danger" : level === "warn" ? "warning" : "normal";
}

/** Lọc không phân biệt hoa thường; ô lọc trống thì mọi dòng đều khớp. */
export function matchesFilter(text: string, filter: string): boolean {
  const query = filter.trim().toLowerCase();
  if (query === "") return true;
  return text.replace(ANSI, "").toLowerCase().includes(query);
}
