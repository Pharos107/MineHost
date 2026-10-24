import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Server } from "../../api/client";
import { ServerActions } from "./ServerActions";

function server(patch: Partial<Server>): Server {
  return { id: "01JB", name: "Survival", state: "stopped", eula_accepted: false, restart_pending: false, slow_start: false, ...patch };
}

function mockApi() {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    void init;
    if (url.endsWith("/eula")) return Response.json(server({ eula_accepted: true }));
    return new Response(null, { status: 204 });
  });
  vi.stubGlobal("fetch", fetchMock);
  return () => fetchMock.mock.calls.filter(([, init]) => init?.method === "POST").map(([url]) => url.replace("/api/v1/servers/01JB", ""));
}

function renderActions(s: Server) {
  const client = new QueryClient();
  render(
    <QueryClientProvider client={client}>
      <ServerActions server={s} />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ServerActions", () => {
  it("bật lần đầu thì hỏi EULA, đồng ý xong mới gửi start", async () => {
    const posts = mockApi();
    renderActions(server({}));
    await userEvent.click(screen.getByRole("button", { name: "Bật server" }));
    expect(screen.getByRole("link", { name: "Đọc EULA của Minecraft" }).getAttribute("href")).toBe("https://aka.ms/MinecraftEULA");
    expect(posts()).toEqual([]);

    await userEvent.click(screen.getByRole("button", { name: "Tôi đồng ý" }));
    await vi.waitFor(() => expect(posts()).toEqual(["/eula", "/actions/start"]));
  });

  it("hủy hộp thoại EULA thì không gửi gì", async () => {
    const posts = mockApi();
    renderActions(server({}));
    await userEvent.click(screen.getByRole("button", { name: "Bật server" }));
    await userEvent.click(screen.getByRole("button", { name: "Hủy" }));
    expect(posts()).toEqual([]);
  });

  it("đang chạy thì có Tắt và Khởi động lại; Buộc dừng phải xác nhận", async () => {
    const posts = mockApi();
    renderActions(server({ state: "running", eula_accepted: true }));
    expect(screen.queryByRole("button", { name: "Bật server" })).toBeNull();
    expect(screen.getByRole("button", { name: "Khởi động lại" })).toBeTruthy();

    await userEvent.click(screen.getByRole("button", { name: "Thao tác khác" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Buộc dừng" }));
    expect(posts()).toEqual([]);
    await userEvent.click(screen.getByRole("button", { name: "Buộc dừng" }));
    await vi.waitFor(() => expect(posts()).toEqual(["/actions/kill"]));
  });
});
