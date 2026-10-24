import type { TFunction } from "i18next";

import type { JobProgress } from "./api/client";
import { formatByteProgress } from "./format";

/** Bước Job từ backend, ví dụ `{ "kind": "download_java", "major": 21 }`. */
interface Step {
  kind?: string;
  [param: string]: unknown;
}

/** Ví dụ "Tải Java 21 (48/61 MB)". Bước lạ (backend mới hơn UI) thì hiện chung "Đang xử lý". */
export function jobStepLabel(t: TFunction, progress: JobProgress, locale: string): string {
  const { kind, ...params } = progress.step as Step;
  const label = t(`jobs.steps.${kind ?? ""}`, { ...params, defaultValue: t("jobs.working") });
  const { bytes_done: done, bytes_total: total } = progress;
  if (done == null || total == null || total <= 0) return label;
  return `${label} (${formatByteProgress(done, total, locale)})`;
}

/** Phần trăm của cả Job: các bước đã xong cộng phần đã tải của bước hiện tại. */
export function jobPercent(progress: JobProgress): number {
  const count = Math.max(progress.step_count, 1);
  const { bytes_done: done, bytes_total: total } = progress;
  const inStep = done != null && total != null && total > 0 ? Math.min(done / total, 1) : 0;
  return Math.round(((progress.step_index - 1 + inStep) / count) * 100);
}
