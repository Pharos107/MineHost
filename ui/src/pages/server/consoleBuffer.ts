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

/** Lọc không phân biệt hoa thường; ô lọc trống thì mọi dòng đều khớp. */
export function matchesFilter(text: string, filter: string): boolean {
  const query = filter.trim().toLowerCase();
  if (query === "") return true;
  return text.replace(ANSI, "").toLowerCase().includes(query);
}
