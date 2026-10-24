import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Trash2 } from "lucide-react";
import { describe, expect, it, vi } from "vitest";

import type { ServerState } from "../api/client";
import i18n from "../i18n";
import { Button, IconButton, Notice, StatusBadge } from ".";

describe("Button", () => {
  it("không cho bấm khi đang loading", async () => {
    const onClick = vi.fn();
    render(
      <Button loading onClick={onClick}>
        Lưu
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Lưu" });
    expect(button).toHaveProperty("disabled", true);
    expect(button.getAttribute("aria-busy")).toBe("true");
    await userEvent.click(button);
    expect(onClick).not.toHaveBeenCalled();
  });

  it("mặc định là type=button để không vô tình gửi form", () => {
    render(<Button>Hủy</Button>);
    expect(screen.getByRole("button").getAttribute("type")).toBe("button");
  });
});

describe("IconButton", () => {
  it("có tên đọc được từ label", () => {
    render(<IconButton icon={Trash2} label="Xóa server" />);
    expect(screen.getByRole("button", { name: "Xóa server" })).toBeTruthy();
  });
});

describe("Notice", () => {
  it("lỗi dùng role=alert để được đọc ngay, loại khác dùng role=status", () => {
    render(
      <>
        <Notice tone="danger">Hỏng</Notice>
        <Notice tone="info">Ghi chú</Notice>
      </>,
    );
    expect(screen.getByRole("alert").textContent).toContain("Hỏng");
    expect(screen.getByRole("status").textContent).toContain("Ghi chú");
  });
});

describe("StatusBadge", () => {
  const states: ServerState[] = ["installing", "stopped", "starting", "running", "stopping", "crashed", "broken"];

  it.each(["vi", "en"])("có nhãn đã dịch cho đủ 7 trạng thái (%s)", async (lang) => {
    await i18n.changeLanguage(lang);
    for (const state of states) {
      const key = `serverState.${state}`;
      expect(i18n.exists(key)).toBe(true);
      const { unmount } = render(<StatusBadge state={state} />);
      expect(screen.getByText(i18n.t(key))).toBeTruthy();
      unmount();
    }
    await i18n.changeLanguage("vi");
  });
});
