import { describe, expect, it } from "vitest";

import { parseMotd } from "./motd";

describe("parseMotd", () => {
  it("tách màu, kiểu chữ và reset khi gặp mã màu mới hoặc §r", () => {
    const lines = parseMotd("§6§l✦ §e§lPROMINENCE II§r thường");
    expect(lines).toEqual([
      [
        { text: "✦ ", color: "#ffaa00", bold: true, italic: false, underline: false, strikethrough: false, obfuscated: false },
        { text: "PROMINENCE II", color: "#ffff55", bold: true, italic: false, underline: false, strikethrough: false, obfuscated: false },
        { text: " thường", bold: false, italic: false, underline: false, strikethrough: false, obfuscated: false },
      ],
    ]);
  });

  it("chỉ lấy tối đa 2 dòng cho chuỗi có xuống dòng và hiểu mã \\u00A7", () => {
    const lines = parseMotd("\\u00A76Dòng 1\\nDòng 2\\nDòng 3");
    expect(lines).toHaveLength(2);
    expect(lines[0]?.[0]?.text).toBe("Dòng 1");
    expect(lines[0]?.[0]?.color).toBe("#ffaa00");
    expect(lines[1]?.[0]?.text).toBe("Dòng 2");
  });
});
