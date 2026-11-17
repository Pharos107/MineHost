import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { CodeEditor } from "./CodeEditor";
import { languageFor } from "./codeLanguage";

describe("languageFor", () => {
  it("đoán ngôn ngữ theo đuôi file, không phân biệt hoa thường", () => {
    expect(languageFor("server.properties")).toBe("properties");
    expect(languageFor("plugins/Essentials/config.yml")).toBe("yaml");
    expect(languageFor("config/sodium.TOML")).toBe("toml");
    expect(languageFor("pack.mcmeta")).toBe("json");
    expect(languageFor("start.sh")).toBe("shell");
    expect(languageFor("logs/latest.log")).toBe("text");
    expect(languageFor(".bashrc")).toBe("text");
    expect(languageFor("Makefile")).toBe("text");
  });
});

describe("CodeEditor", () => {
  it("dựng CodeMirror có nhãn, có số dòng, thay nội dung khi giá trị đổi từ ngoài", async () => {
    const onChange = vi.fn();
    const { rerender } = render(<CodeEditor label="Nội dung a.yml" value={"a: 1\nb: 2\n"} onChange={onChange} language="yaml" status={<span>Đã lưu</span>} />);
    const content = await screen.findByRole("textbox", { name: "Nội dung a.yml" });
    expect(content.getAttribute("contenteditable")).toBe("true");
    expect(content.textContent).toContain("a: 1");
    expect(document.querySelector(".cm-lineNumbers")).toBeTruthy();
    expect(screen.getByText("Đã lưu")).toBeTruthy();

    rerender(<CodeEditor label="Nội dung a.yml" value="c: 3" onChange={onChange} language="yaml" />);
    await waitFor(() => expect(content.textContent).toBe("c: 3"));
    // Nội dung đổi từ ngoài vẫn báo lại để trang biết bản đang hiện.
    expect(onChange).toHaveBeenLastCalledWith("c: 3");
  });
});
