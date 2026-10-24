const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

/** Ví dụ: 17179869184 → "16 GB" (đơn vị 1024). */
export function formatBytes(bytes: number, locale: string): string {
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = value >= 10 || unit === 0 ? 0 : 1;
  const number = new Intl.NumberFormat(locale, { maximumFractionDigits: digits }).format(value);
  return `${number} ${UNITS[unit]}`;
}

/** RAM của server tính bằng MB, ví dụ 4096 → "4 GB", 1536 → "1,5 GB" (vi). */
export function formatMemoryMb(mb: number, locale: string): string {
  return formatBytes(mb * 1024 * 1024, locale);
}
