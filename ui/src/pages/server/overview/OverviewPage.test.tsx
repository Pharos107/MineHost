import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen } from "@testing-library/react";
import { MemoryRouter, Outlet, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import i18n from "../../../i18n";
import type { MetricSample, Server, ServerMetrics } from "../../../api/client";
import { formatUptime } from "./duration";
import { appendSample, MAX_SAMPLES } from "./metricsLive";
import { OverviewPage } from "./OverviewPage";

type Handler = (message: { data: unknown; error?: unknown }) => void;
const handlers = new Map<string, Handler>();

vi.mock("../../../api/ws", () => ({
  live: {
    subscribe: (topic: string, handler: Handler) => {
      handlers.set(topic, handler);
      return () => handlers.delete(topic);
    },
  },
}));

const GB = 1024 ** 3;

function server(patch: Partial<Server>): Server {
  return { id: "A", name: "Survival", state: "running", eula_accepted: true, restart_pending: false, slow_start: false, memory_mb: { min: 1024, max: 4096 }, ...patch };
}

function sample(at: string, cpu: number, memory: number): MetricSample {
  return { at, cpu_percent: cpu, memory_bytes: memory, players: null };
}

function renderPage(s: Server, metrics: ServerMetrics) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) => (url === "/api/v1/servers/A/metrics" ? Response.json(metrics) : Response.json({}, { status: 404 }))),
  );
  // `OverviewPage` đọc server từ `Outlet` của `ServerLayout`; ở đây dựng một layout giả.
  function Layout() {
    return <Routes><Route element={<OutletWith server={s} />}><Route index element={<OverviewPage />} /></Route></Routes>;
  }
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <MemoryRouter>
        <Layout />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function OutletWith({ server: s }: { server: Server }) {
  return <Outlet context={s} />;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("OverviewPage", () => {
  it("server chạy: thẻ số theo mẫu mới nhất, có biểu đồ CPU và RAM, mẫu mới qua WebSocket", async () => {
    const now = new Date().toISOString();
    renderPage(server({}), { samples: [sample(now, 12.5, 2.5 * GB)], disk_bytes: 5 * GB, running_since: new Date(Date.now() - 3_900_000).toISOString() });
    expect(await screen.findByText("12,5")).toBeTruthy();
    expect(screen.getByText("2,5 GB")).toBeTruthy();
    expect(screen.getByText("trên 4 GB đã đặt")).toBeTruthy();
    expect(screen.getByText("5 GB")).toBeTruthy();
    expect(screen.getByText("1 giờ 5 phút")).toBeTruthy();
    expect(screen.getByRole("img", { name: "CPU trong 1 giờ qua: 13 %" })).toBeTruthy();
    act(() => handlers.get("server.A.metrics")?.({ data: sample(new Date().toISOString(), 40, 3 * GB) }));
    expect(await screen.findByText("40")).toBeTruthy();
  });

  it("server tắt: không vẽ biểu đồ, ghi rõ server đang tắt", async () => {
    renderPage(server({ state: "stopped" }), { samples: [sample(new Date().toISOString(), 10, GB)], disk_bytes: null, running_since: null });
    expect(await screen.findByText("Server đang tắt. Bật server để xem CPU và RAM.")).toBeTruthy();
    expect(screen.queryByRole("img")).toBeNull();
    expect(screen.getByText("Đang tính…")).toBeTruthy();
  });
});

describe("chỉ số", () => {
  it("giữ tối đa 720 mẫu", () => {
    const many = Array.from({ length: MAX_SAMPLES }, (_, i) => sample(String(i), i, 0));
    const next = appendSample({ samples: many, disk_bytes: null, running_since: null }, sample("x", 1, 0));
    expect(next?.samples).toHaveLength(MAX_SAMPLES);
    expect(next?.samples[0]?.at).toBe("1");
    expect(next?.samples.at(-1)?.at).toBe("x");
  });

  it("thời gian chạy lấy hai đơn vị lớn nhất", () => {
    const t = i18n.getFixedT("vi");
    expect(formatUptime(t, 45)).toBe("45 giây");
    expect(formatUptime(t, 8040)).toBe("2 giờ 14 phút");
    expect(formatUptime(t, 3600)).toBe("1 giờ");
    expect(formatUptime(t, 266_400)).toBe("3 ngày 2 giờ");
  });
});
