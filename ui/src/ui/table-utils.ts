export type SortDirection = "asc" | "desc";

export interface SortState {
  columnId: string;
  direction: SortDirection;
}

/**
 * Bỏ dấu và viết thường để tìm "may chu" vẫn ra "Máy chủ". `đ` không tách được bằng NFD
 * nên đổi riêng.
 */
export function foldText(text: string): string {
  return text
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .replace(/đ/g, "d")
    .replace(/Đ/g, "D")
    .toLowerCase()
    .trim();
}

export function filterRows<T>(rows: T[], query: string, searchText: (row: T) => string): T[] {
  const needle = foldText(query);
  if (needle === "") return rows;
  return rows.filter((row) => foldText(searchText(row)).includes(needle));
}

/** Sắp xếp ổn định; chuỗi so theo ngôn ngữ đang dùng và hiểu số ("server 2" trước "server 10"). */
export function sortRows<T>(
  rows: T[],
  sortValue: (row: T) => string | number,
  direction: SortDirection,
  locale: string,
): T[] {
  const collator = new Intl.Collator(locale, { numeric: true, sensitivity: "base" });
  const sign = direction === "asc" ? 1 : -1;
  return [...rows].sort((a, b) => {
    const x = sortValue(a);
    const y = sortValue(b);
    const order = typeof x === "number" && typeof y === "number" ? x - y : collator.compare(String(x), String(y));
    return order * sign;
  });
}

/** Bấm lại cột đang sắp thì đảo chiều; bấm cột khác thì sắp tăng dần theo cột đó. */
export function nextSort(current: SortState | null, columnId: string): SortState {
  if (current?.columnId === columnId) {
    return { columnId, direction: current.direction === "asc" ? "desc" : "asc" };
  }
  return { columnId, direction: "asc" };
}
