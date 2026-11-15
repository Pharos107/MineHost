import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Job, Server } from "../../api/client";
import { HomePage } from "./HomePage";
import { homeStats } from "./stats";

vi.mock("../../api/ws", () => ({ live: { subscribe: () => () => {} } }));

const GB = 1024 ** 3;

const players = {
  running: true,
  online_mode: true,
  whitelist_enabled: false,
  max_players: 20,
  players: [{ name: "Steve", uuid: null, online: true, last_seen: null, whitelisted: false, op: false, banned: false, ban_reason: null, frozen: false }],
};

function server(patch: Partial<Server>): Server {
  return { id: "A", name: "Survival", state: "running", eula_accepted: true, restart_pending: false, slow_start: false, ignored_jvm_args: false, preflight_blocking: false, port: 25565, ...patch };
}

const system = {
  cpu_percent: 23.6,
  memory_used_bytes: 12 * GB,
  memory_total_bytes: 32 * GB,
  disk_free_bytes: 300 * GB,
  lan_addresses: ["192.168.1.5"],
};

function renderPage(servers: Server[], jobs: Job[] = []) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) => {
      if (url === "/api/v1/servers") return Response.json(servers);
      if (url === "/api/v1/jobs") return Response.json(jobs);
      if (url === "/api/v1/system") return Response.json(system);
      if (url === "/api/v1/servers/A/players") return Response.json(players);
      return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
    }),
  );
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <MemoryRouter>
        <HomePage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("HomePage", () => {
  it("dải số liệu máy ở trên; mỗi server một thẻ, server đang chạy lên trước", async () => {
    renderPage([server({ id: "B", name: "Creative", state: "stopped", port: 25566 }), server({ id: "A", name: "Survival" })]);
    expect(await screen.findByText("1 trên 2 server đang chạy.")).toBeTruthy();
    const titles = screen.getAllByRole("article").map((card) => within(card).getByRole("heading", { level: 2 }).textContent);
    expect(titles).toEqual(["Survival", "Creative"]);
    expect(screen.getByText("192.168.1.5:25565")).toBeTruthy();
    // Thẻ server đang chạy hiện ai đang online.
    expect(await screen.findByRole("group", { name: "Steve" })).toBeTruthy();
    expect(screen.getByText("1 / 20")).toBeTruthy();
    const machine = screen.getByRole("region", { name: "Máy này" });
    expect(await within(machine).findByText("23,6 %")).toBeTruthy();
    expect(within(machine).getByText("12 GB / 32 GB")).toBeTruthy();
  });

  it("không server nào chạy thì ghi rõ", async () => {
    renderPage([server({ state: "stopped" })]);
    expect(await screen.findByText(/Chưa server nào chạy/)).toBeTruthy();
  });

  it("có tác vụ thì hiện mục Hoạt động gần đây", async () => {
    renderPage([server({ state: "stopped" })], [{ id: 7, kind: "backup", state: "succeeded", started_at: "2026-11-12T10:00:00Z", server_id: "A" }]);
    const table = await screen.findByRole("table", { name: "Hoạt động gần đây" });
    expect(within(table).getByText("Sao lưu")).toBeTruthy();
  });

  it("chưa có server thì có nút tạo server đầu tiên", async () => {
    renderPage([]);
    expect(await screen.findByRole("button", { name: "Tạo server đầu tiên" })).toBeTruthy();
  });
});

describe("homeStats", () => {
  it("đếm server có process và Job đang chạy; chưa có thông tin máy thì để null", () => {
    const stats = homeStats(
      [server({ state: "starting" }), server({ state: "crashed" })],
      [{ id: 1, kind: "create_server", state: "running", started_at: "" }],
      undefined,
    );
    expect(stats).toMatchObject({ running: 1, total: 2, jobsRunning: 1, cpuPercent: null, memoryUsed: null });
  });
});
