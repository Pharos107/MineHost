import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useImperativeHandle, useState, type Ref } from "react";
import { MemoryRouter, Outlet, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import i18n from "../../../i18n";
import type { MetricSample, Server, ServerMetrics } from "../../../api/client";
import type { TerminalHandle } from "../../../ui";
import { formatUptime } from "./duration";
import { appendSample, MAX_SAMPLES } from "./metricsLive";
import { OverviewPage } from "./OverviewPage";
import { backupText } from "./ServerInfoCard";

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

// xterm cần canvas mà jsdom không có: thay bằng danh sách dòng, như test của trang Console.
vi.mock("../../../ui/Terminal", () => ({
  Terminal: function FakeTerminal({ label, ref }: { label: string; ref?: Ref<TerminalHandle> }) {
    const [shown, setShown] = useState<string[]>([]);
    useImperativeHandle(ref, () => ({ write: (text: string) => setShown((s) => [...s, text]), clear: () => setShown([]) }));
    return <ul aria-label={label}>{shown.map((text, i) => <li key={i}>{text}</li>)}</ul>;
  },
}));

const GB = 1024 ** 3;
const player = (name: string, online: boolean) => ({ name, online, op: false, banned: false, frozen: false, whitelisted: true });
const PLAYERS = { max_players: 20, online_mode: true, running: true, whitelist_enabled: true, players: [player("Steve", true), player("Alex", true), player("Notch", false)] };

function server(patch: Partial<Server>): Server {
  return { id: "A", name: "Survival", state: "running", eula_accepted: true, restart_pending: false, slow_start: false, ignored_jvm_args: false, preflight_blocking: false, memory_mb: { min: 1024, max: 4096 }, ...patch };
}

function sample(at: string, cpu: number, memory: number, players: number | null = null): MetricSample {
  return { at, cpu_percent: cpu, memory_bytes: memory, players };
}

function metricsPayload(patch: Partial<ServerMetrics>): ServerMetrics {
  return { samples: [], disk_bytes: null, running_since: null, retention_minutes: 60, step_seconds: 5, ...patch };
}

function renderPage(s: Server, metrics: Partial<ServerMetrics>) {
  const payload = metricsPayload(metrics);
  const fetchSpy = vi.fn(async (url: string, init?: RequestInit) => {
    if (url.startsWith("/api/v1/servers/A/metrics")) return Response.json(payload);
    if (url === "/api/v1/servers/A/players") return Response.json(PLAYERS);
    if (url === "/api/v1/servers/A" && init?.method === "PATCH") {
      const body = JSON.parse(String(init.body)) as { metrics_retention_minutes?: number };
      return Response.json({ ...s, metrics_retention_minutes: body.metrics_retention_minutes ?? s.metrics_retention_minutes });
    }
    return Response.json({}, { status: 404 });
  });
  vi.stubGlobal("fetch", fetchSpy);
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
  return fetchSpy;
}

function OutletWith({ server: s }: { server: Server }) {
  return <Outlet context={s} />;
}

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
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

  it("thẻ Người chơi chỉ hiện số đang chơi trên tối đa; RAM có thanh mức", async () => {
    renderPage(server({}), { samples: [sample(new Date().toISOString(), 5, 2.5 * GB)], disk_bytes: null, running_since: null });
    expect(await screen.findByText("/ 20")).toBeTruthy();
    expect(screen.getByText("2")).toBeTruthy();
    expect(screen.queryByText("Steve")).toBeNull();
    expect(screen.getByRole("meter", { name: "RAM" }).getAttribute("aria-valuenow")).toBe("63");
  });

  it("đổi cách xếp biểu đồ thì nhớ lại; Xem chi tiết mở hộp có số trung bình, cao nhất", async () => {
    const now = Date.now();
    const at = (ago: number) => new Date(now - ago).toISOString();
    renderPage(server({}), { samples: [sample(at(10_000), 10, GB), sample(at(5_000), 30, GB), sample(at(0), 20, GB)], disk_bytes: null, running_since: null });
    const stacked = await screen.findByRole("radio", { name: "Từng hàng" });
    expect(document.querySelector("[data-columns]")?.getAttribute("data-columns")).toBe("3");
    await userEvent.click(stacked);
    expect(document.querySelector("[data-columns]")?.getAttribute("data-columns")).toBe("1");
    expect(localStorage.getItem("minehost.overview.charts")).toBe("stacked");

    await userEvent.click(screen.getByRole("button", { name: "Mở chi tiết: CPU trong 1 giờ qua" }));
    const dialog = screen.getByRole("dialog", { name: "CPU trong 1 giờ qua" });
    expect(dialog.textContent).toContain("Trung bình20 %");
    expect(dialog.textContent).toContain("Cao nhất30 %");
    expect(dialog.textContent).toContain("Thấp nhất10 %");
    expect(screen.getByRole("combobox", { name: "Chọn ngày xem" })).toBeTruthy();
  });

  it("bấm bánh răng mở cài đặt biểu đồ, đổi khung hiển thị và thời gian lưu server", async () => {
    const now = new Date().toISOString();
    const fetchSpy = renderPage(server({ metrics_retention_minutes: 60 }), { samples: [sample(now, 15, GB)] });
    await screen.findByRole("img", { name: "CPU trong 1 giờ qua: 15 %" });

    await userEvent.click(screen.getAllByRole("button", { name: "Cài đặt biểu đồ" })[0]!);
    const dialog = screen.getByRole("dialog", { name: "Cài đặt: CPU trong 1 giờ qua" });
    expect(dialog).toBeTruthy();
    expect(screen.getByRole("button", { name: "Hủy" })).toBeTruthy();
    expect(screen.queryByLabelText("Thời lượng tùy chỉnh")).toBeNull();

    await userEvent.click(screen.getAllByRole("button", { name: "Tùy chỉnh" })[1]!);
    expect(screen.getByRole("spinbutton", { name: "Thời lượng tùy chỉnh" })).toBeTruthy();

    await userEvent.click(screen.getAllByRole("button", { name: "30 phút" })[0]!);
    await userEvent.click(screen.getAllByRole("button", { name: "1 ngày" })[1]!);
    await userEvent.click(screen.getByRole("button", { name: "Lưu" }));

    expect(await screen.findByRole("img", { name: "CPU trong 30 phút qua: 15 %" })).toBeTruthy();
    const patchCall = fetchSpy.mock.calls.find(([u, init]) => u === "/api/v1/servers/A" && init?.method === "PATCH");
    expect(patchCall).toBeTruthy();
    expect(JSON.parse(String(patchCall?.[1]?.body))).toEqual({ metrics_retention_minutes: 1440 });
  });

  it("thẻ Thông tin server, biểu đồ Người chơi và vài dòng console cuối", async () => {
    const s = server({ java_major: 21, port: 25565, autostart: true, restart_on_crash: false, restart_at: ["04:00"], backup: { schedule: "every_6h", keep: 5 } });
    renderPage(s, { samples: [sample(new Date().toISOString(), 5, GB, 2)], disk_bytes: null, running_since: null });
    expect(await screen.findByRole("img", { name: "Người chơi trong 1 giờ qua: 2" })).toBeTruthy();
    const info = screen.getByText("Sao lưu tự động").closest("dl")!;
    expect(info.textContent).toContain("Java21");
    expect(info.textContent).toContain("RAM1 GB – 4 GB");
    expect(info.textContent).toContain("Tự bật cùng MineHostCó");
    expect(info.textContent).toContain("Tự bật lại khi crashKhông");
    expect(info.textContent).toContain("Khởi động lại hằng ngày04:00");
    expect(info.textContent).toContain("Sao lưu tự độngMỗi 6 giờ");
    act(() => handlers.get("server.A.console")?.({ data: { seq: 1, stream: "stdout", text: "Steve joined the game" } }));
    expect(screen.getByRole("list", { name: "Console gần đây" }).textContent).toContain("Steve joined the game");
  });

  it("server tắt: không vẽ biểu đồ, ghi rõ server đang tắt", async () => {
    renderPage(server({ state: "stopped" }), { samples: [sample(new Date().toISOString(), 10, GB)], disk_bytes: null, running_since: null });
    expect(await screen.findByText("Server đang tắt. Bật server để xem CPU và RAM.")).toBeTruthy();
    expect(screen.queryByRole("img")).toBeNull();
    expect(screen.getByRole("heading", { name: "Thông tin server" })).toBeTruthy();
    expect(screen.getByText("Đang tính…")).toBeTruthy();
  });
});

describe("chỉ số", () => {
  it("giữ tối đa 720 mẫu", () => {
    const many = Array.from({ length: MAX_SAMPLES }, (_, i) => sample(String(i), i, 0));
    const next = appendSample(metricsPayload({ samples: many }), sample("x", 1, 0));
    expect(next?.samples).toHaveLength(MAX_SAMPLES);
    expect(next?.samples[0]?.at).toBe("1");
    expect(next?.samples.at(-1)?.at).toBe("x");
  });

  it("lịch sao lưu thành câu đọc được", () => {
    const t = i18n.getFixedT("vi");
    expect(backupText(t, "daily@03:30")).toBe("Hằng ngày lúc 03:30");
    expect(backupText(t, "every_1h")).toBe("Mỗi 1 giờ");
    expect(backupText(t, "off")).toBe("Không");
    expect(backupText(t, undefined)).toBe("Không");
  });

  it("thời gian chạy lấy hai đơn vị lớn nhất", () => {
    const t = i18n.getFixedT("vi");
    expect(formatUptime(t, 45)).toBe("45 giây");
    expect(formatUptime(t, 8040)).toBe("2 giờ 14 phút");
    expect(formatUptime(t, 3600)).toBe("1 giờ");
    expect(formatUptime(t, 266_400)).toBe("3 ngày 2 giờ");
  });
});
