/**
 * Lịch sử lệnh của ô console, giống terminal: mũi tên lên xem lệnh cũ hơn, xuống về lệnh mới
 * hơn rồi về đúng chỗ đang gõ dở. Hàm thuần để test không cần giao diện.
 */
export interface CommandHistory {
  entries: string[];
  /** Vị trí đang xem; bằng `entries.length` là đang ở dòng gõ mới. */
  index: number;
  /** Chữ đang gõ dở trước khi bấm mũi tên lên. */
  draft: string;
}

const MAX_ENTRIES = 100;

export const emptyHistory: CommandHistory = { entries: [], index: 0, draft: "" };

/** Thêm lệnh vừa gửi; bỏ qua lệnh rỗng và lệnh trùng lệnh ngay trước. */
export function pushCommand(history: CommandHistory, command: string): CommandHistory {
  const trimmed = command.trim();
  const last = history.entries[history.entries.length - 1];
  const entries = trimmed === "" || trimmed === last ? history.entries : [...history.entries, trimmed].slice(-MAX_ENTRIES);
  return { entries, index: entries.length, draft: "" };
}

export function historyUp(history: CommandHistory, current: string): { history: CommandHistory; value: string } {
  if (history.entries.length === 0) return { history, value: current };
  const draft = history.index === history.entries.length ? current : history.draft;
  const index = Math.max(0, history.index - 1);
  return { history: { ...history, index, draft }, value: history.entries[index] ?? current };
}

export function historyDown(history: CommandHistory, current: string): { history: CommandHistory; value: string } {
  if (history.index >= history.entries.length) return { history, value: current };
  const index = history.index + 1;
  const value = index === history.entries.length ? history.draft : (history.entries[index] ?? current);
  return { history: { ...history, index }, value };
}
