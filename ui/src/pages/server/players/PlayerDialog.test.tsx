import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Server } from "../../../api/client";
import type { Players } from "../../../api/players";
import { PlayersPage } from "./PlayersPage";

vi.mock("../../../api/ws", () => ({ live: { subscribe: () => () => {} } }));
// jsdom không có WebGL; thay skin 3D bằng một ô có nhãn để biết đã được dựng.
vi.mock("./SkinViewer", () => ({ default: ({ label }: { label: string }) => <canvas role="img" aria-label={label} /> }));

function makePlayers(running: boolean): Players {
  return {
    running,
    online_mode: true,
    whitelist_enabled: true,
    max_players: 20,
    players: [
      { name: "Steve", uuid: "u-steve", online: running, last_seen: null, whitelisted: false, op: false, banned: false, ban_reason: null, frozen: false },
      { name: "Alex", uuid: "u-alex", online: running, last_seen: null, whitelisted: true, op: false, banned: false, ban_reason: null, frozen: false },
    ],
  };
}

function setup(running: boolean, list: () => Players = () => makePlayers(running)) {
  const server: Server = { id: "A", name: "Survival", state: running ? "running" : "stopped", eula_accepted: true, restart_pending: false, slow_start: false, ignored_jvm_args: false };
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    if (url === "/api/v1/servers/A/players") return Response.json(list());
    if (url.endsWith("/actions") && init?.method === "POST") return Response.json({ done: running ? "sent" : "saved" });
    return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
  });
  vi.stubGlobal("fetch", fetchMock);
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
  return fetchMock;
}

function lastBody(fetchMock: ReturnType<typeof setup>, name: string): unknown {
  const call = fetchMock.mock.calls.filter(([url]) => url === `/api/v1/servers/A/players/${name}/actions`).at(-1);
  return JSON.parse(String(call?.[1]?.body));
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("PlayerDialog", () => {
  it("server tắt: nút tương tác mờ, cấm vẫn gửi kèm lý do", async () => {
    const fetchMock = setup(false);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /Chi tiết.*Steve/ }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByRole("img", { name: /Skin của Steve/ })).toBeTruthy();
    expect(within(dialog).getByText(/Server đang tắt: chỉ đổi được/)).toBeTruthy();
    for (const name of ["Đuổi", "Đóng băng", "Về điểm spawn", "Dịch chuyển", "Nhắn tin"]) {
      expect(within(dialog).getByRole("button", { name })).toHaveProperty("disabled", true);
    }
    await user.type(within(dialog).getByRole("textbox", { name: "Lý do" }), "phá nhà");
    await user.click(within(dialog).getByRole("button", { name: "Cấm" }));
    await waitFor(() => expect(lastBody(fetchMock, "Steve")).toEqual({ action: "ban", reason: "phá nhà" }));
    expect(await within(dialog).findByText(/Đã lưu/)).toBeTruthy();
  });

  it("server chạy: dịch chuyển tới người khác và nhắn tin", async () => {
    const fetchMock = setup(true);
    const user = userEvent.setup();
    const online = await screen.findByRole("table", { name: "Đang online" });
    await user.click(within(online).getByRole("button", { name: /Chi tiết.*Steve/ }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByRole("combobox", { name: "Dịch chuyển tới" })).toHaveProperty("value", "Alex");
    await user.click(within(dialog).getByRole("button", { name: "Dịch chuyển" }));
    await waitFor(() => expect(lastBody(fetchMock, "Steve")).toEqual({ action: "teleport_to_player", target: "Alex" }));

    const send = within(dialog).getByRole("button", { name: "Nhắn tin" });
    expect(send).toHaveProperty("disabled", true);
    await user.type(within(dialog).getByRole("textbox", { name: "Nội dung" }), "chào");
    await user.click(send);
    await waitFor(() => expect(lastBody(fetchMock, "Steve")).toEqual({ action: "message", text: "chào" }));
    expect(await within(dialog).findByText(/Đã gửi lệnh/)).toBeTruthy();
  });

  it("bỏ cấm người chỉ có trong danh sách cấm thì hộp thoại vẫn mở", async () => {
    let banned = true;
    const griefer = { name: "Griefer", uuid: "u-g", online: false, last_seen: null, whitelisted: false, op: false, banned: true, ban_reason: "phá", frozen: false };
    setup(false, () => ({ ...makePlayers(false), players: banned ? [griefer] : [] }));
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /Chi tiết.*Griefer/ }));
    const dialog = await screen.findByRole("dialog");
    banned = false;
    await user.click(within(dialog).getByRole("button", { name: "Bỏ cấm" }));
    expect(await within(screen.getByRole("dialog")).findByRole("button", { name: "Cấm" })).toBeTruthy();
    expect(within(screen.getByRole("dialog")).getByText(/Đã lưu/)).toBeTruthy();
  });
});
