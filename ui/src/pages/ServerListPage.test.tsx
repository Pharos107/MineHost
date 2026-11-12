import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Server } from "../api/client";
import { ServerListPage } from "./ServerListPage";

function mockApi(servers: Server[]) {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    if (init?.method === "POST") return new Response(null, { status: 204 });
    if (url === "/api/v1/servers") return Response.json(servers);
    if (url === "/api/v1/system") return Response.json({ lan_addresses: ["192.168.1.5"] });
    return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
  });
  vi.stubGlobal("fetch", fetchMock);
  // Các URL đã POST, bỏ phần đầu chung cho dễ đọc.
  return () => fetchMock.mock.calls.filter(([, init]) => init?.method === "POST").map(([url]) => url.replace("/api/v1/servers/", ""));
}

function server(patch: Partial<Server>): Server {
  return { id: "01JB", name: "Survival", state: "stopped", eula_accepted: true, restart_pending: false, slow_start: false, ...patch };
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
  localStorage.clear();
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
    // Mặc định là thẻ.
    const card = (await screen.findByRole("link", { name: "Survival" })).closest("article")!;
    expect(within(card).getByText("Đang chạy")).toBeTruthy();
    expect(within(card).getByText("Paper 1.21.4, tối đa 4 GB RAM")).toBeTruthy();
    // Bảng: lựa chọn được nhớ cho lần mở sau.
    await userEvent.click(screen.getByRole("radio", { name: "Bảng" }));
    expect(within(screen.getByRole("table")).getByText("Paper 1.21.4")).toBeTruthy();
    expect(screen.getByText("4 GB")).toBeTruthy();
    expect(localStorage.getItem("minehost.servers.view")).toBe("table");
    await userEvent.click(screen.getByRole("link", { name: "Survival" }));
    expect(screen.getByText("chi tiết")).toBeTruthy();
  });

  it("lọc theo trạng thái; lọc ra trống vẫn đổi lại được", async () => {
    mockApi([server({ id: "A", name: "Chạy", state: "running" }), server({ id: "B", name: "Tắt", state: "stopped" })]);
    renderPage();
    await screen.findByRole("link", { name: "Chạy" });
    const select = screen.getByRole("combobox", { name: "Lọc theo trạng thái" });
    await userEvent.selectOptions(select, "running");
    expect(screen.queryByRole("link", { name: "Tắt" })).toBeNull();
    await userEvent.selectOptions(select, "problem");
    expect(screen.getByText("Không có server nào ở trạng thái này")).toBeTruthy();
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Lọc theo trạng thái" }), "all");
    expect(screen.getByRole("link", { name: "Tắt" })).toBeTruthy();
  });

  it("nút trên dòng gửi đúng thao tác; chưa đồng ý EULA thì không có nút Bật", async () => {
    const posts = mockApi([
      server({ id: "A", name: "Đang chạy", state: "running", port: 25565 }),
      server({ id: "B", name: "Mới tạo", eula_accepted: false }),
    ]);
    localStorage.setItem("minehost.servers.view", "table");
    renderPage();
    expect(await screen.findByText("192.168.1.5:25565")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Tắt: Đang chạy" }));
    expect(posts()).toEqual(["A/actions/stop"]);
    expect(screen.queryByRole("button", { name: "Bật server: Mới tạo" })).toBeNull();
  });
});
