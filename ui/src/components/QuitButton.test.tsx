import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Server } from "../api/client";
import { QuitButton } from "./QuitButton";

function server(id: string, state: Server["state"]): Server {
  return { id, name: id, state, eula_accepted: true, restart_pending: false, slow_start: false, ignored_jvm_args: false, preflight_blocking: false };
}

/** Giả lập controller: nhận lệnh Thoát xong thì web còn trả lời một lần rồi tắt hẳn. */
function mockApi(servers: Server[]) {
  let quitting = false;
  let healthAfterQuit = 0;
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    if (url.endsWith("/app/shutdown") && init?.method === "POST") {
      quitting = true;
      return new Response(null, { status: 202 });
    }
    if (url.endsWith("/health")) {
      if (quitting && ++healthAfterQuit > 1) throw new TypeError("Failed to fetch");
      return Response.json({ status: "ok", version: "0.1.0" });
    }
    return Response.json(servers);
  });
  vi.stubGlobal("fetch", fetchMock);
  return () => fetchMock.mock.calls.some(([url, init]) => url.endsWith("/app/shutdown") && init?.method === "POST");
}

function renderButton() {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <QuitButton />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("QuitButton", () => {
  it("không có server chạy thì thoát ngay và báo khi MineHost đã tắt", async () => {
    const quitSent = mockApi([server("a", "stopped")]);
    renderButton();
    await vi.waitFor(() => expect(screen.getByRole("button", { name: "Thoát" })).toHaveProperty("disabled", false));
    await userEvent.click(screen.getByRole("button", { name: "Thoát" }));
    expect(quitSent()).toBe(true);
    expect(await screen.findByText(/MineHost đã tắt/, undefined, { timeout: 3000 })).toBeTruthy();
  });

  it("còn server chạy thì hỏi lại, Hủy thì không gửi gì", async () => {
    const quitSent = mockApi([server("a", "running"), server("b", "starting"), server("c", "stopped")]);
    renderButton();
    await vi.waitFor(() => expect(screen.getByRole("button", { name: "Thoát" })).toHaveProperty("disabled", false));
    await userEvent.click(screen.getByRole("button", { name: "Thoát" }));
    expect(screen.getByText("Có 2 server đang chạy. Tắt an toàn tất cả rồi thoát?")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Hủy" }));
    expect(quitSent()).toBe(false);

    await userEvent.click(screen.getByRole("button", { name: "Thoát" }));
    await userEvent.click(screen.getByRole("button", { name: "Tắt an toàn và thoát" }));
    await vi.waitFor(() => expect(quitSent()).toBe(true));
    expect(await screen.findByText("Đã tắt 0/2 server")).toBeTruthy();
  });
});
