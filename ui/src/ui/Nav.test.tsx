import { render, screen } from "@testing-library/react";
import { House, Server, Terminal } from "lucide-react";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import { NavItem, NavSection } from "./Nav";

function renderAt(path: string) {
  render(
    <MemoryRouter initialEntries={[path]}>
      <NavSection title="Survival">
        <NavItem to="/" end icon={House} label="Trang chủ" />
        <NavItem to="/servers/1" end icon={Server} label="Tổng quan" />
        <NavItem to="/servers/1/console" icon={Terminal} label="Console" />
      </NavSection>
    </MemoryRouter>,
  );
}

describe("NavItem", () => {
  it("chỉ mục khớp đường dẫn hiện tại có aria-current", () => {
    renderAt("/servers/1/console");
    expect(screen.getByRole("link", { name: "Console" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getByRole("link", { name: "Tổng quan" }).getAttribute("aria-current")).toBeNull();
    expect(screen.getByRole("link", { name: "Trang chủ" }).getAttribute("aria-current")).toBeNull();
  });

  it("mục có `end` sáng khi đường dẫn trùng khớp hoàn toàn", () => {
    renderAt("/servers/1");
    expect(screen.getByRole("link", { name: "Tổng quan" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getByRole("link", { name: "Console" }).getAttribute("aria-current")).toBeNull();
  });

  it("NavSection hiện tiêu đề nhóm", () => {
    renderAt("/");
    expect(screen.getByText("Survival")).toBeTruthy();
  });
});
