import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Server } from "../../../api/client";
import type { Players } from "../../../api/players";
import { Avatar } from "../../../ui";
import { PlayersPage } from "./PlayersPage";

vi.mock("../../../api/ws", () => ({ live: { subscribe: () => () => {} } }));

const server: Server = { id: "A", name: "Survival", state: "stopped", eula_accepted: true, restart_pending: false, slow_start: false, ignored_jvm_args: false };

const players: Players = {
  running: false,
  online_mode: true,
  whitelist_enabled: false,
  max_players: 20,
  players: [
    { name: "Steve", uuid: "u-steve", online: true, last_seen: "2026-11-12T10:00:00Z", whitelisted: true, op: true, banned: false, ban_reason: null, frozen: true },
    { name: "Griefer", uuid: "u-griefer", online: false, last_seen: null, whitelisted: false, op: false, banned: true, ban_reason: "phá nhà", frozen: false },
  ],
};

function mockFetch() {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    if (url === "/api/v1/servers/A/players") return Response.json(players);
    if (url.endsWith("/actions") && init?.method === "POST") return Response.json({ done: "saved" });
    return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

function renderPage() {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <MemoryRouter initialEntries={["/servers/A/players"]}>
        <Routes>
          <Route path="/servers/:id" element={<Outlet context={server} />}>
            <Route path="players" element={<PlayersPage />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("PlayersPage", () => {
  it("chia hai bảng và hiện huy hiệu", async () => {
    mockFetch();
    renderPage();
    const online = await screen.findByRole("table", { name: "Đang online" });
    expect(within(online).getByText("Steve")).toBeTruthy();
    for (const badge of ["Op", "Whitelist", "Đóng băng"]) expect(within(online).getByText(badge)).toBeTruthy();
    const known = screen.getByRole("table", { name: "Đã từng vào" });
    expect(within(known).getByText("Griefer")).toBeTruthy();
    expect(within(known).getByText("Bị cấm")).toBeTruthy();
    expect(within(known).getByText("Chưa vào")).toBeTruthy();
    expect(screen.getByText("Đang online (1/20)")).toBeTruthy();
    expect(screen.getByText(/Server đang tắt/)).toBeTruthy();
    expect(screen.getByText(/Whitelist đang tắt/)).toBeTruthy();
  });

  it("thêm theo tên gửi đúng thao tác và chặn tên sai", async () => {
    const fetchMock = mockFetch();
    const user = userEvent.setup();
    renderPage();
    const input = screen.getByRole("textbox", { name: "Tên người chơi" });
    await user.type(input, "ab");
    expect(screen.getByText(/Tên chỉ gồm/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Thêm vào whitelist" })).toHaveProperty("disabled", true);

    await user.clear(input);
    await user.type(input, "Alex_1");
    await user.click(screen.getByRole("button", { name: "Cấm" }));
    await waitFor(() => {
      const post = fetchMock.mock.calls.find(([url]) => url === "/api/v1/servers/A/players/Alex_1/actions");
      expect(JSON.parse(String(post?.[1]?.body))).toEqual({ action: "ban" });
    });
    expect(await screen.findByText("Đã cấm Alex_1.")).toBeTruthy();
  });
});

describe("Avatar", () => {
  it("hiện chữ cái cho tới khi skin tải xong", () => {
    const { container } = render(<Avatar name="steve" skinUrl="/skin.png" />);
    const avatar = screen.getByRole("img", { name: "steve" });
    expect(avatar.textContent).toBe("S");
    const probe = container.querySelector("img");
    expect(probe).not.toBeNull();
    fireEvent.load(probe!);
    expect(avatar.textContent).toBe("");
    expect(container.querySelector("img")).toBeNull();
  });

  it("không có skin thì chỉ có chữ", () => {
    const { container } = render(<Avatar name="Alex" />);
    expect(screen.getByRole("img", { name: "Alex" }).textContent).toBe("A");
    expect(container.querySelector("img")).toBeNull();
  });
});
