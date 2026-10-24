import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Server } from "../api/client";
import { ServerListPage } from "./ServerListPage";

function mockApi(servers: Server[]) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) => {
      if (url === "/api/v1/servers") return Response.json(servers);
      return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
    }),
  );
}

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/"]}>
        <Routes>
          <Route path="/" element={<ServerListPage />} />
          <Route path="/new" element={<p>wizard</p>} />
          <Route path="/servers/:id" element={<p>chi tiết</p>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ServerListPage", () => {
  it("chưa có server thì có nút lớn dẫn tới wizard", async () => {
    mockApi([]);
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "Tạo server đầu tiên" }));
    expect(screen.getByText("wizard")).toBeTruthy();
  });

  it("hiện server kèm trạng thái, tên dẫn tới trang chi tiết", async () => {
    mockApi([
      {
        id: "01JB",
        name: "Survival",
        state: "running",
        loader: "paper",
        minecraft: "1.21.4",
        memory_mb: { min: 1024, max: 4096 },
        port: 25565,
        eula_accepted: true,
        restart_pending: false,
        slow_start: false,
      },
    ]);
    renderPage();
    const link = await screen.findByRole("link", { name: "Survival" });
    expect(screen.getByText("Đang chạy")).toBeTruthy();
    expect(screen.getByText("Paper 1.21.4")).toBeTruthy();
    expect(screen.getByText("4 GB")).toBeTruthy();
    await userEvent.click(link);
    expect(screen.getByText("chi tiết")).toBeTruthy();
  });
});
