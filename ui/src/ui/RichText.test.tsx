import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { RichText, markdownToHtml, sanitizeHtml } from "./RichText";

describe("RichText", () => {
  it("chuyển markdown của Modrinth, giữ định dạng, bảng, ảnh https và link mở tab mới", () => {
    const html = markdownToHtml(
      "# Lithium\n\n**Nhanh** hơn.\n\n| A | B |\n| - | - |\n| 1 | 2 |\n\n![logo](https://cdn.modrinth.com/a.png)\n\n[Wiki](https://wiki.example.com)",
    );
    const doc = new DOMParser().parseFromString(html, "text/html");
    expect(doc.querySelector("h1")?.textContent).toBe("Lithium");
    expect(doc.querySelector("strong")?.textContent).toBe("Nhanh");
    expect(doc.querySelector("table td")?.textContent).toBe("1");
    expect(doc.querySelector("img")?.getAttribute("src")).toBe("https://cdn.modrinth.com/a.png");
    const link = doc.querySelector("a");
    expect(link?.getAttribute("target")).toBe("_blank");
    expect(link?.getAttribute("rel")).toBe("noopener noreferrer");
  });

  it("bỏ script, iframe, thuộc tính on*, style, ảnh http và link javascript:", () => {
    const html = sanitizeHtml(
      '<p onclick="x()" style="color:red">Chữ</p><script>alert(1)</script><iframe src="https://evil"></iframe>' +
        '<img src="http://cdn/a.png" onerror="x()"><img src="https://cdn/b.png" onerror="x()">' +
        '<a href="javascript:alert(1)">bấm</a><form><input></form><style>p{}</style>',
    );
    expect(html).not.toMatch(/script|iframe|onclick|onerror|style|javascript:|<form|<input|http:\/\/cdn/);
    expect(html).toContain("Chữ");
    expect(html).toContain('src="https://cdn/b.png"');
    expect(html).toContain(">bấm</a>");
  });

  it("hiện HTML của CurseForge trong khung có nhãn", () => {
    render(<RichText source="<p>JEI <b>xem</b> công thức</p>" format="html" label="Mô tả" />);
    expect(screen.getByLabelText("Mô tả").textContent).toBe("JEI xem công thức");
  });
});
