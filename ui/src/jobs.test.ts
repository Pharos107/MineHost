import { describe, expect, it } from "vitest";

import type { JobProgress } from "./api/client";
import i18n from "./i18n";
import { jobPercent, jobStepLabel } from "./jobs";

const MB = 1024 * 1024;

function progress(step: object, index: number, done?: number, total?: number): JobProgress {
  return { step: step as JobProgress["step"], step_index: index, step_count: 5, bytes_done: done, bytes_total: total };
}

describe("jobStepLabel", () => {
  const label = (p: JobProgress) => jobStepLabel(i18n.t, p, "vi");

  it("ghép tham số của bước và số đã tải", () => {
    expect(label(progress({ kind: "download_java", major: 21 }, 3, 48 * MB, 61 * MB))).toBe("Tải Java 21 (48/61 MB)");
    expect(label(progress({ kind: "download_server", minecraft: "1.21.4" }, 4))).toBe("Tải Paper 1.21.4");
    expect(label(progress({ kind: "prepare_files" }, 5))).toBe("Chuẩn bị file");
  });

  it("bước UI chưa biết thì hiện chữ chung", () => {
    expect(label(progress({ kind: "download_mods" }, 2))).toBe("Đang xử lý…");
  });
});

describe("jobPercent", () => {
  it("tính cả phần đã tải của bước đang làm", () => {
    expect(jobPercent(progress({ kind: "check_disk" }, 1))).toBe(0);
    expect(jobPercent(progress({ kind: "download_java" }, 3, 50, 100))).toBe(50);
    expect(jobPercent(progress({ kind: "prepare_files" }, 5))).toBe(80);
  });
});
