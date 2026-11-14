import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Job, Server } from "../../api/client";
import { JobsPage } from "./JobsPage";
import { upsertJob } from "./jobsLive";

type Handler = (message: { data: unknown; error?: unknown }) => void;
const handlers = new Map<string, Handler>();

vi.mock("../../api/ws", () => ({
  live: {
    subscribe: (topic: string, handler: Handler) => {
      handlers.set(topic, handler);
      return () => handlers.delete(topic);
    },
  },
}));

const servers: Server[] = [
  { id: "A", name: "Survival", state: "running", eula_accepted: true, restart_pending: false, slow_start: false, ignored_jvm_args: false, preflight_blocking: false },
  { id: "B", name: "Creative", state: "stopped", eula_accepted: true, restart_pending: false, slow_start: false, ignored_jvm_args: false, preflight_blocking: false },
];

function job(patch: Partial<Job>): Job {
  return { id: 1, kind: "create_server", state: "succeeded", server_id: "A", started_at: "2026-10-27T01:00:00Z", ...patch };
}

function renderPage(jobs: Job[]) {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    if (init?.method === "POST") return new Response(null, { status: 204 });
    if (url === "/api/v1/jobs") return Response.json(jobs);
    if (url === "/api/v1/servers") return Response.json(servers);
    return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
  });
  vi.stubGlobal("fetch", fetchMock);
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <MemoryRouter>
        <JobsPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return fetchMock;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("JobsPage", () => {
  it("hiện Job với tên server, trạng thái và lỗi đã dịch", async () => {
    renderPage([
      job({ id: 2, state: "failed", error: { code: "NOT_ENOUGH_DISK", params: {} } }),
      job({ id: 1, server_id: "B" }),
    ]);
    const table = await screen.findByRole("table");
    expect(await within(table).findByRole("link", { name: "Survival" })).toBeTruthy();
    expect(within(table).getByText("Lỗi")).toBeTruthy();
    expect(within(table).getByText("Xong")).toBeTruthy();
    expect(within(table).getAllByText("Tạo server")).toHaveLength(2);
  });

  it("chỉ Job đang chạy có nút Hủy, bấm thì gửi yêu cầu hủy", async () => {
    const fetchMock = renderPage([job({ id: 3, state: "running" }), job({ id: 1 })]);
    await userEvent.click(await screen.findByRole("button", { name: "Hủy: #3" }));
    expect(screen.queryByRole("button", { name: "Hủy: #1" })).toBeNull();
    const posts = fetchMock.mock.calls.filter(([, init]) => init?.method === "POST").map(([url]) => url);
    expect(posts).toEqual(["/api/v1/jobs/3/cancel"]);
  });

  it("lọc theo server và cập nhật theo topic jobs", async () => {
    renderPage([job({ id: 1, server_id: "B" })]);
    await screen.findByRole("table");
    act(() => handlers.get("jobs")?.({ data: job({ id: 5, server_id: "A", state: "running" }) }));
    expect(await screen.findByText("Đang chạy")).toBeTruthy();
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Lọc theo server" }), "B");
    expect(screen.queryByText("Đang chạy")).toBeNull();
  });

  it("chưa có Job thì hiện màn hình trống", async () => {
    renderPage([]);
    expect(await screen.findByText("Chưa có tác vụ nào")).toBeTruthy();
  });
});

describe("upsertJob", () => {
  it("thay Job cũ cùng id và giữ mới nhất trước", () => {
    const list = upsertJob([job({ id: 2 }), job({ id: 1 })], job({ id: 1, state: "failed" }));
    expect(list.map((j) => [j.id, j.state])).toEqual([
      [2, "succeeded"],
      [1, "failed"],
    ]);
    expect(upsertJob(list, job({ id: 9 }))[0]?.id).toBe(9);
  });
});
