const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

function unitFor(bytes: number): number {
  let unit = 0;
  while (bytes >= 1024 ** (unit + 1) && unit < UNITS.length - 1) unit += 1;
  return unit;
}

function formatIn(bytes: number, unit: number, locale: string): string {
  const value = bytes / 1024 ** unit;
  const digits = value >= 10 || unit === 0 ? 0 : 1;
  return new Intl.NumberFormat(locale, { maximumFractionDigits: digits }).format(value);
}

/** Ví dụ: 17179869184 → "16 GB" (đơn vị 1024). */
export function formatBytes(bytes: number, locale: string): string {
  const unit = unitFor(bytes);
  return `${formatIn(bytes, unit, locale)} ${UNITS[unit]}`;
}

/** Tiến độ tải, chung một đơn vị theo tổng: "48/61 MB". */
export function formatByteProgress(done: number, total: number, locale: string): string {
  const unit = unitFor(total);
  return `${formatIn(done, unit, locale)}/${formatIn(total, unit, locale)} ${UNITS[unit]}`;
}

/** RAM của server tính bằng MB, ví dụ 4096 → "4 GB", 1536 → "1,5 GB" (vi). */
export function formatMemoryMb(mb: number, locale: string): string {
  return formatBytes(mb * 1024 * 1024, locale);
}
