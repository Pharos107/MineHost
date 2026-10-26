import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Job, Server } from "../../api/client";
import { HomePage } from "./HomePage";
import { homeStats } from "./stats";

vi.mock("../../api/ws", () => ({ live: { subscribe: () => () => {} } }));

const GB = 1024 ** 3;

function server(patch: Partial<Server>): Server {
  return { id: "A", name: "Survival", state: "running", eula_accepted: true, restart_pending: false, slow_start: false, port: 25565, ...patch };
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
  it("thẻ số và danh sách chỉ gồm server đang chạy", async () => {
    renderPage([server({ id: "A", name: "Survival" }), server({ id: "B", name: "Creative", state: "stopped" })]);
    expect(await screen.findByText("23,6")).toBeTruthy();
    expect(screen.getByText("/ 2")).toBeTruthy();
    expect(screen.getByText("12 GB")).toBeTruthy();
    expect(screen.getByText("trên 32 GB")).toBeTruthy();
    const table = await screen.findByRole("table", { name: "Server đang chạy" });
    expect(within(table).getByRole("link", { name: "Survival" })).toBeTruthy();
    expect(within(table).queryByRole("link", { name: "Creative" })).toBeNull();
    expect(within(table).getByText("192.168.1.5:25565")).toBeTruthy();
  });

  it("không server nào chạy thì ghi rõ", async () => {
    renderPage([server({ state: "stopped" })]);
    expect(await screen.findByText("Chưa có server nào đang chạy.")).toBeTruthy();
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
