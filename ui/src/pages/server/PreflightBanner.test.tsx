import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Preflight, Server } from "../../api/client";
import { PreflightBanner } from "./PreflightBanner";

function server(patch: Partial<Server> = {}): Server {
  return {
    id: "01JB",
    name: "Modded",
    state: "stopped",
    content_kind: "mods",
    eula_accepted: true,
    restart_pending: false,
    slow_start: false,
    ignored_jvm_args: false,
    preflight_blocking: false,
    ...patch,
  };
}

function renderBanner(s: Server, preflight: Preflight) {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    void init;
    if (url.endsWith("/preflight")) return Response.json(preflight);
    if (url.endsWith("/actions/start")) return new Response(null, { status: 204 });
    return new Response(null, { status: 404 });
  });
  vi.stubGlobal("fetch", fetchMock);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <PreflightBanner server={s} />
    </QueryClientProvider>,
  );
  return fetchMock;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("PreflightBanner", () => {
  it("hiện lỗi chặn và nút Vẫn bật gửi ignore_preflight", async () => {
    const fetchMock = renderBanner(server({ preflight_blocking: true }), {
      missing_deps: [{ id: "fabric-api", version_req: ">=0.100.0", required_by: "Sodium" }],
      version_mismatches: [],
      wrong_loader: [],
      pending: [],
      client_only: [{ file_name: "zoom.jar", name: "Zoomify" }],
    });

    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByText(/Thiếu fabric-api >=0\.100\.0 \(do Sodium yêu cầu\)/)).toBeTruthy();
    expect(screen.getByText(/Zoomify chỉ chạy ở máy người chơi/)).toBeTruthy();

    await userEvent.click(screen.getByRole("button", { name: "Vẫn bật" }));
    await vi.waitFor(() => {
      const startCall = fetchMock.mock.calls.find(([url]) => url === "/api/v1/servers/01JB/actions/start");
      expect(startCall).toBeTruthy();
      expect(JSON.parse(String(startCall?.[1]?.body))).toEqual({ ignore_preflight: true });
    });
  });

  it("chỉ có cảnh báo thì hiện status và không có nút Vẫn bật", async () => {
    renderBanner(server(), {
      missing_deps: [],
      version_mismatches: [{ id: "cloth", version_req: ">=15.0", installed_version: "14.0", required_by: "Iris" }],
      wrong_loader: [{ file_name: "bad.jar", name: "Bad", formats: ["forge"] }],
      pending: [],
      client_only: [],
    });

    expect(await screen.findByRole("status")).toBeTruthy();
    expect(screen.getByText(/Iris cần cloth >=15\.0, hiện có 14\.0/)).toBeTruthy();
    expect(screen.getByText(/Bad \(bad\.jar\) không khớp loại server hiện tại/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Vẫn bật" })).toBeNull();
  });

  it("bỏ qua server Vanilla và server đang chạy", () => {
    const vanillaFetch = renderBanner(server({ content_kind: null }), {
      missing_deps: [{ id: "x", version_req: "*", required_by: "Y" }],
      version_mismatches: [],
      wrong_loader: [],
      pending: [],
      client_only: [],
    });
    expect(vanillaFetch).not.toHaveBeenCalled();

    const runningFetch = renderBanner(server({ state: "running", preflight_blocking: true }), {
      missing_deps: [{ id: "x", version_req: "*", required_by: "Y" }],
      version_mismatches: [],
      wrong_loader: [],
      pending: [],
      client_only: [],
    });
    expect(runningFetch).toHaveBeenCalledTimes(0);
  });
});
