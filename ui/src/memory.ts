import type { components } from "./api/schema";

type Memory = components["schemas"]["MemoryDto"];

const MB = 1024 * 1024;
export const MEMORY_STEP_MB = 512;
/** Phần RAM luôn chừa lại cho hệ điều hành (spec P1, mục 4.3). */
const RESERVED_MB = 2048;

/**
 * Giới hạn thanh trượt RAM: cho chọn tới `total - 2048` MB, theo bội số 512. Máy quá ít RAM
 * thì vẫn cho chọn tới mức đề xuất để thanh trượt không bị rỗng.
 */
export function memoryRange(totalBytes: number, suggested: Memory): { min: number; max: number } {
  const totalMb = Math.floor(totalBytes / MB);
  const usable = Math.floor((totalMb - RESERVED_MB) / MEMORY_STEP_MB) * MEMORY_STEP_MB;
  return { min: Math.min(1024, suggested.max), max: Math.max(usable, suggested.max) };
}

/** RAM gửi cho backend khi người dùng chọn mức tối đa `maxMb`: `min = min(1024, max)`. */
export function memoryFor(maxMb: number): Memory {
  return { min: Math.min(1024, maxMb), max: maxMb };
}

/** Chọn nhiều hơn RAM đang trống thì cảnh báo vàng nhưng vẫn cho tạo. */
export function exceedsAvailable(maxMb: number, availableBytes: number): boolean {
  return maxMb * MB > availableBytes;
}
