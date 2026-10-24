import { describe, expect, it } from "vitest";

import { formatByteProgress, formatBytes, formatMemoryMb } from "./format";

const MB = 1024 * 1024;

describe("format", () => {
  it("formatBytes chọn đơn vị theo 1024", () => {
    expect(formatBytes(512, "vi")).toBe("512 B");
    expect(formatBytes(16 * 1024 * MB, "vi")).toBe("16 GB");
    expect(formatBytes(1.5 * 1024 * MB, "en")).toBe("1.5 GB");
  });

  it("formatMemoryMb đổi MB ra chữ theo ngôn ngữ", () => {
    expect(formatMemoryMb(4096, "vi")).toBe("4 GB");
    expect(formatMemoryMb(1536, "vi")).toBe("1,5 GB");
    expect(formatMemoryMb(512, "en")).toBe("512 MB");
  });

  it("formatByteProgress dùng chung đơn vị của tổng", () => {
    expect(formatByteProgress(48 * MB, 61 * MB, "vi")).toBe("48/61 MB");
    expect(formatByteProgress(512 * 1024, 61 * MB, "en")).toBe("0.5/61 MB");
    expect(formatByteProgress(0, 2 * 1024 * MB, "en")).toBe("0/2 GB");
  });
});
