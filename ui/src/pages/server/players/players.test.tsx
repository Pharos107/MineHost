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

const server: Server = { id: "A", name: "Survival", state: "stopped", eula_accepted: true, restart_pending: false, slow_start: false, ignored_jvm_args: false, preflight_blocking: false };

const players: Players = {
  running: false,
  online_mode: true,
  whitelist_enabled: false,
  max_players: 20,
  players: [
    { name: "Steve", uuid: "u-steve", online: true, last_seen: "2026-11-12T10:00:00Z", whitelisted: true, op: true, banned: false, ban_reason: null, frozen: true },
    { name: "Alex", uuid: "u-alex", online: false, last_seen: "2026-11-12T09:00:00Z", whitelisted: true, op: false, banned: false, ban_reason: null, frozen: false },
    { name: "Griefer", uuid: "u-griefer", online: false, last_seen: null, whitelisted: false, op: false, banned: true, ban_reason: "phá nhà", frozen: false },
  ],
};

function mockFetch(data: Players = players) {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    if (url === "/api/v1/servers/A/players") return Response.json(data);
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

/** Tên các hàng trong danh sách, theo thứ tự hiện. */
function rowNames(): string[] {
  const list = screen.getByRole("region", { name: "Người chơi" });
  return within(list)
    .queryAllByRole("listitem")
    .map((li) => within(li).getAllByRole("img")[0]?.getAttribute("aria-label") ?? "");
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("PlayersPage", () => {
  it("ô số, nhóm Đang chơi / Không online và huy hiệu", async () => {
    mockFetch();
    renderPage();
    const list = await screen.findByRole("region", { name: "Người chơi" });
    expect(await within(list).findByText("Steve")).toBeTruthy();
    expect(within(list).getByRole("heading", { name: /Đang chơi\s*1/ })).toBeTruthy();
    expect(within(list).getByRole("heading", { name: /Không online\s*2/ })).toBeTruthy();
    const steve = within(list).getByRole("button", { name: /Steve/ });
    for (const badge of ["Op", "Đóng băng"]) expect(within(steve).getByText(badge)).toBeTruthy();
    // Hàng trong danh sách bỏ huy hiệu whitelist cho tên có chỗ.
    expect(within(steve).queryByText("Whitelist")).toBeNull();
    expect(within(within(list).getByRole("button", { name: /Griefer/ })).getByText("Bị cấm")).toBeTruthy();
    expect(within(list).getByRole("button", { name: /Griefer/ }).textContent).toContain("Chưa vào");

    // Ô số: 1/20 đang chơi, 2 whitelist (đang tắt), 1 op, 1 bị cấm.
    expect(screen.getByText("/ 20")).toBeTruthy();
    expect(screen.getByText("Đang tắt")).toBeTruthy();
    expect(screen.getByText(/Server đang tắt/)).toBeTruthy();
    expect(screen.getByText("Chọn một người chơi")).toBeTruthy();
  });

  it("server chưa có ai thì danh sách là ô trống, không có nhóm", async () => {
    mockFetch({ ...players, players: [] });
    renderPage();
    expect(await screen.findByText("Chưa có người chơi nào")).toBeTruthy();
    expect(screen.queryByRole("heading", { name: /Đang chơi/ })).toBeNull();
  });

  it("lọc theo nhóm và tìm theo tên", async () => {
    mockFetch();
    const user = userEvent.setup();
    renderPage();
    await screen.findByText("Steve");
    await user.click(screen.getByRole("radio", { name: "Bị cấm" }));
    expect(rowNames()).toEqual(["Griefer"]);
    await user.click(screen.getByRole("radio", { name: "Whitelist" }));
    expect(rowNames()).toEqual(["Steve", "Alex"]);
    await user.click(screen.getByRole("radio", { name: "Tất cả" }));
    await user.type(screen.getByRole("searchbox", { name: "Tìm theo tên" }), "al");
    expect(rowNames()).toEqual(["Alex"]);
    await user.clear(screen.getByRole("searchbox", { name: "Tìm theo tên" }));
    await user.type(screen.getByRole("searchbox", { name: "Tìm theo tên" }), "zzz");
    expect(screen.getByText("Không có ai khớp.")).toBeTruthy();
  });

  it("bấm một người thì khung chi tiết hiện đúng người đó", async () => {
    mockFetch();
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole("button", { name: /Griefer/ }));
    expect(screen.getByRole("button", { name: /Griefer/ }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("heading", { level: 2, name: "Griefer" })).toBeTruthy();
    expect(screen.getByText("phá nhà")).toBeTruthy();
    expect(screen.queryByText("Chọn một người chơi")).toBeNull();
  });

  it("thêm theo tên trong hộp thoại gửi đúng thao tác và chặn tên sai", async () => {
    const fetchMock = mockFetch();
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole("button", { name: "Thêm" }));
    const dialog = await screen.findByRole("dialog", { name: "Thêm người chơi" });
    expect(within(dialog).getByText(/Whitelist đang tắt/)).toBeTruthy();
    const input = within(dialog).getByRole("textbox", { name: "Tên người chơi" });
    await user.type(input, "ab");
    expect(within(dialog).getByText(/Tên chỉ gồm/)).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "Thêm vào whitelist" })).toHaveProperty("disabled", true);

    await user.clear(input);
    await user.type(input, "Alex_1");
    await user.click(within(dialog).getByRole("button", { name: "Cấm" }));
    await waitFor(() => {
      const post = fetchMock.mock.calls.find(([url]) => url === "/api/v1/servers/A/players/Alex_1/actions");
      expect(JSON.parse(String(post?.[1]?.body))).toEqual({ action: "ban" });
    });
    expect(await within(dialog).findByText("Đã cấm Alex_1.")).toBeTruthy();
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
