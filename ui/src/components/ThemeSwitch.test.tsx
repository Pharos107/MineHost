import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";

import { ThemeSwitch } from "./ThemeSwitch";

afterEach(() => {
  localStorage.clear();
  delete document.documentElement.dataset.theme;
});

describe("ThemeSwitch", () => {
  it("chuyển qua lại sáng và tối, nhớ lựa chọn trên trình duyệt", async () => {
    const user = userEvent.setup();
    render(<ThemeSwitch />);
    // jsdom không có chế độ tối của hệ điều hành: mặc định là sáng.
    await user.click(screen.getByRole("button", { name: "Chuyển sang giao diện tối" }));
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem("minehost.theme")).toBe("dark");
    await user.click(screen.getByRole("button", { name: "Chuyển sang giao diện sáng" }));
    expect(document.documentElement.dataset.theme).toBe("light");
  });
});
