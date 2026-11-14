import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Server } from "./api/client";
import { AppRoutes } from "./App";

// xterm cần canvas mà jsdom không có; ở đây chỉ kiểm tra route đưa đúng server vào trang con.
vi.mock("./pages/server/ConsoleTab", () => ({
  ConsoleTab: ({ server }: { server: Server }) => <p>console của {server.name}</p>,
}));

const server: Server = {
  id: "01JB",
  name: "Survival",
  state: "running",
  content_kind: "mods",
  eula_accepted: true,
  restart_pending: false,
  slow_start: false,
  ignored_jvm_args: false,
  preflight_blocking: false,
};

function renderAt(path: string, currentServer: Server = server) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) => {
      if (url === "/api/v1/servers") return Response.json([currentServer]);
      if (url === "/api/v1/servers/01JB") return Response.json(currentServer);
      if (url === "/api/v1/servers/01JB/content") return Response.json({ kind: currentServer.content_kind ?? "mods", files: [], pending: [] });
      if (url === "/api/v1/system") return Response.json({ lan_addresses: [] });
      return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
    }),
  );
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[path]}>
        <AppRoutes />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("AppRoutes", () => {
  it("console là trang riêng trong server, thanh bên chuyển sang các mục của server", async () => {
    renderAt("/servers/01JB/console");
    expect(await screen.findByText("console của Survival")).toBeTruthy();
    const nav = screen.getByRole("navigation", { name: "Điều hướng chính" });
    expect(within(nav).getByRole("link", { name: "Console" }).getAttribute("aria-current")).toBe("page");
    expect(within(nav).getByRole("link", { name: "Tất cả máy chủ" }).getAttribute("href")).toBe("/servers");
  });

  it("đường dẫn cũ của P1 /servers/:id vẫn mở được, giờ là Tổng quan", async () => {
    renderAt("/servers/01JB");
    expect(await screen.findByRole("heading", { level: 1, name: "Survival" })).toBeTruthy();
    expect(await screen.findByText("Thời gian chạy")).toBeTruthy();
    const nav = screen.getByRole("navigation", { name: "Điều hướng chính" });
    expect(within(nav).getByRole("link", { name: "Tổng quan" }).getAttribute("aria-current")).toBe("page");
  });

  it("trang chung dùng thanh bên chính", async () => {
    renderAt("/jobs");
    expect(await screen.findByRole("heading", { level: 1, name: "Tác vụ" })).toBeTruthy();
    const nav = screen.getByRole("navigation", { name: "Điều hướng chính" });
    expect(within(nav).getByRole("link", { name: "Tác vụ" }).getAttribute("aria-current")).toBe("page");
    expect(within(nav).queryByRole("link", { name: "Console" })).toBeNull();
  });

  it("wizard không có thanh bên", async () => {
    renderAt("/new");
    expect(await screen.findByRole("heading", { level: 1, name: "Tạo server" })).toBeTruthy();
    expect(screen.queryByRole("navigation", { name: "Điều hướng chính" })).toBeNull();
  });

  it("thanh bên hiện Mod khi content_kind = mods và mở trang Mod", async () => {
    renderAt("/servers/01JB/content");
    expect(await screen.findByText("Chưa có mod nào")).toBeTruthy();
    const nav = screen.getByRole("navigation", { name: "Điều hướng chính" });
    expect(within(nav).getByRole("link", { name: "Mod" }).getAttribute("aria-current")).toBe("page");
  });

  it("thanh bên hiện Plugin khi content_kind = plugins", async () => {
    renderAt("/servers/01JB", { ...server, content_kind: "plugins" });
    expect(await screen.findByRole("link", { name: "Plugin" })).toBeTruthy();
  });

  it("thanh bên ẩn mục Mod/Plugin với Vanilla", async () => {
    renderAt("/servers/01JB", { ...server, content_kind: null });
    await screen.findByRole("heading", { level: 1, name: "Survival" });
    expect(screen.queryByRole("link", { name: "Mod" })).toBeNull();
    expect(screen.queryByRole("link", { name: "Plugin" })).toBeNull();
  });
});
