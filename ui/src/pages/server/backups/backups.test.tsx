import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Backup } from "../../../api/backups";
import type { Server } from "../../../api/client";
import { BackupsPage } from "./BackupsPage";
import { scheduleErrors, scheduleForm, scheduleString } from "./schedule";

vi.mock("../../../api/ws", () => ({ live: { subscribe: () => () => {} } }));

const server: Server = {
  id: "A",
  name: "Survival",
  state: "running",
  eula_accepted: true,
  restart_pending: false,
  slow_start: false,
  ignored_jvm_args: false,
  preflight_blocking: false,
  backup: { schedule: "every_6h", keep: 10 },
};

const backups: Backup[] = [
  { id: "01B", kind: "manual", created_at: "2026-11-12T10:00:00Z", size: 2048, minecraft: "1.21.4", loader: "paper", note: "Trước khi đổi", protected: true },
  { id: "01A", kind: "scheduled", created_at: "2026-11-12T04:00:00Z", size: 1024, minecraft: "1.21.4", loader: "paper", note: null, protected: false },
];

function mockFetch() {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    const method = init?.method ?? "GET";
    if (url === "/api/v1/servers/A/backups" && method === "GET") return Response.json(backups);
    if (url === "/api/v1/servers/A/backups" && method === "POST") return Response.json({ job_id: 5 }, { status: 202 });
    if (url.endsWith("/restore")) return Response.json({ job_id: 6 }, { status: 202 });
    if (url.startsWith("/api/v1/jobs/")) return Response.json({ id: 5, kind: "backup", state: "running", started_at: "" });
    return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

function renderPage() {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <MemoryRouter initialEntries={["/servers/A/backups"]}>
        <Routes>
          <Route path="/servers/:id" element={<Outlet context={server} />}>
            <Route path="backups" element={<BackupsPage />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("schedule", () => {
  it("đổi qua lại giữa chuỗi của backend và ô nhập", () => {
    expect(scheduleForm("every_6h", 10)).toMatchObject({ mode: "every", hours: "6" });
    expect(scheduleForm("daily@04:30", 3)).toMatchObject({ mode: "daily", time: "04:30", keep: "3" });
    expect(scheduleString({ mode: "daily", hours: "6", time: "04:30", keep: "3" })).toBe("daily@04:30");
    expect(scheduleString(scheduleForm("off", 10))).toBe("off");
    expect(scheduleErrors({ mode: "every", hours: "169", time: "04:00", keep: "10" }).hours).toBe("backups.hoursInvalid");
    expect(scheduleErrors({ mode: "off", hours: "", time: "", keep: "0" }).keep).toBe("backups.keepInvalid");
  });
});

describe("BackupsPage", () => {
  it("sao lưu ngay gửi ghi chú và khóa bảo vệ", async () => {
    const fetchMock = mockFetch();
    const user = userEvent.setup();
    renderPage();
    await user.type(screen.getByRole("textbox", { name: "Ghi chú" }), "Trước khi cài plugin");
    await user.click(screen.getByText("Bảo vệ bản này", { selector: "span" }));
    await user.click(screen.getByRole("button", { name: "Sao lưu" }));
    await waitFor(() => {
      const post = fetchMock.mock.calls.find(([url, init]) => url === "/api/v1/servers/A/backups" && init?.method === "POST");
      expect(JSON.parse(String(post?.[1]?.body))).toEqual({ note: "Trước khi cài plugin", protected: true });
    });
  });

  it("khôi phục phải tích Tôi hiểu; bản được bảo vệ không xóa được", async () => {
    const fetchMock = mockFetch();
    const user = userEvent.setup();
    renderPage();
    const table = await screen.findByRole("table");
    expect(within(table).getByText("Trước khi đổi")).toBeTruthy();
    // Hàng đầu là bản được bảo vệ: trong menu, Xóa bị tắt, Bỏ bảo vệ thay cho Bảo vệ.
    await user.click(within(table).getAllByRole("button", { name: /Thao tác khác cho/ })[0]!);
    expect(screen.getByRole("menuitem", { name: "Xóa" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("menuitem", { name: "Bỏ bảo vệ" })).toBeTruthy();
    await user.keyboard("{Escape}");

    await user.click(within(table).getAllByRole("button", { name: /Khôi phục/ })[1]!);
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/Server đang chạy sẽ được tắt/)).toBeTruthy();
    const confirm = within(dialog).getByRole("button", { name: "Khôi phục" });
    expect(confirm).toHaveProperty("disabled", true);
    await user.click(within(dialog).getByRole("checkbox", { name: "Tôi hiểu" }));
    await user.click(confirm);
    await waitFor(() => expect(fetchMock.mock.calls.some(([url]) => url === "/api/v1/servers/A/backups/01A/restore")).toBe(true));
  });
});
