import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
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
  const server: Server = { id: "A", name: "Survival", state: running ? "running" : "stopped", eula_accepted: true, restart_pending: false, slow_start: false, ignored_jvm_args: false, preflight_blocking: false };
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

async function open(name: string) {
  const user = userEvent.setup();
  await user.click(await screen.findByRole("button", { name: new RegExp(name) }));
  return user;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("PlayerDetail", () => {
  it("server tắt: nút tương tác mờ, cấm vẫn gửi kèm lý do", async () => {
    const fetchMock = setup(false);
    const user = await open("Steve");
    expect(await screen.findByRole("img", { name: /Skin của Steve/ })).toBeTruthy();
    expect(screen.getByText(/Server đang tắt: chỉ đổi được/)).toBeTruthy();
    for (const name of ["Đuổi", "Đóng băng", "Về điểm spawn", "Dịch chuyển", "Nhắn tin"]) {
      expect(screen.getByRole("button", { name })).toHaveProperty("disabled", true);
    }
    await user.type(screen.getByRole("textbox", { name: "Lý do" }), "phá nhà");
    await user.click(screen.getByRole("button", { name: "Cấm" }));
    await waitFor(() => expect(lastBody(fetchMock, "Steve")).toEqual({ action: "ban", reason: "phá nhà" }));
    expect(await screen.findByText(/Đã lưu/)).toBeTruthy();
  });

  it("công tắc whitelist và op gửi đúng thao tác theo trạng thái", async () => {
    const fetchMock = setup(false);
    const user = await open("Alex");
    const whitelist = screen.getByRole("switch", { name: /Trong whitelist/ });
    expect(whitelist).toHaveProperty("checked", true);
    await user.click(whitelist);
    await waitFor(() => expect(lastBody(fetchMock, "Alex")).toEqual({ action: "whitelist_remove" }));
    const op = screen.getByRole("switch", { name: /^Op/ });
    expect(op).toHaveProperty("checked", false);
    await user.click(op);
    await waitFor(() => expect(lastBody(fetchMock, "Alex")).toEqual({ action: "op" }));
  });

  it("server chạy: dịch chuyển tới người khác và nhắn tin", async () => {
    const fetchMock = setup(true);
    const user = await open("Steve");
    expect(screen.getByRole("combobox", { name: "Dịch chuyển tới" })).toHaveProperty("value", "Alex");
    await user.click(screen.getByRole("button", { name: "Dịch chuyển" }));
    await waitFor(() => expect(lastBody(fetchMock, "Steve")).toEqual({ action: "teleport_to_player", target: "Alex" }));

    const send = screen.getByRole("button", { name: "Nhắn tin" });
    expect(send).toHaveProperty("disabled", true);
    await user.type(screen.getByRole("textbox", { name: "Nội dung" }), "chào");
    await user.click(send);
    await waitFor(() => expect(lastBody(fetchMock, "Steve")).toEqual({ action: "message", text: "chào" }));
    expect(await screen.findByText(/Đã gửi lệnh/)).toBeTruthy();
  });

  it("bỏ cấm người chỉ có trong danh sách cấm thì khung chi tiết vẫn mở", async () => {
    let banned = true;
    const griefer = { name: "Griefer", uuid: "u-g", online: false, last_seen: null, whitelisted: false, op: false, banned: true, ban_reason: "phá", frozen: false };
    setup(false, () => ({ ...makePlayers(false), players: banned ? [griefer, ...makePlayers(false).players] : makePlayers(false).players }));
    const user = await open("Griefer");
    banned = false;
    await user.click(screen.getByRole("button", { name: "Bỏ cấm" }));
    expect(await screen.findByRole("button", { name: "Cấm" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2, name: "Griefer" })).toBeTruthy();
    expect(screen.getByText(/Đã lưu/)).toBeTruthy();
  });
});
