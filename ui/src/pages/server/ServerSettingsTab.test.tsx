import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Server } from "../../api/client";
import { ServerSettingsTab } from "./ServerSettingsTab";

const base: Server = {
  id: "01JB",
  name: "Survival",
  state: "stopped",
  eula_accepted: true,
  restart_pending: false,
  slow_start: false,
  ignored_jvm_args: false,
  preflight_blocking: false,
  port: 25565,
  memory_mb: { min: 1024, max: 2048 },
  autostart: false,
  restart_on_crash: true,
  restart_at: [],
};

function renderTab(server: Server) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) =>
      url === "/api/v1/system"
        ? Response.json({
            java_majors: [8, 11, 16, 17, 21, 25],
            memory_total_bytes: 8 * 1024 ** 3,
            suggested_memory_mb: { min: 1024, max: 2048 },
          })
        : Response.json({}),
    ),
  );
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MemoryRouter>
        <ServerSettingsTab server={server} />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ServerSettingsTab · Java", () => {
  it("server 1.17 hiện đúng Java 16 do Mojang yêu cầu, không cảnh báo", async () => {
    renderTab({
      ...base,
      minecraft: "1.17.1",
      java_major: 16,
      java_recommended: 16,
    });
    await userEvent.click(screen.getByRole("button", { name: "Nâng cao" }));
    const select = screen.getByRole("combobox", { name: "Phiên bản Java" });
    expect(
      await screen.findByRole("option", { name: "Java 16 (đề xuất)" }),
    ).toBeTruthy();
    expect(select).toHaveProperty("value", "16");
    expect(screen.queryByText(/thường dùng Java/)).toBeNull();
  });

  it("đã đổi Java khác bản Mojang thì mở sẵn Nâng cao và cảnh báo theo bản Mojang", async () => {
    renderTab({
      ...base,
      minecraft: "1.21.4",
      java_major: 17,
      java_recommended: 21,
    });
    expect(
      await screen.findByRole("option", { name: "Java 21 (đề xuất)" }),
    ).toBeTruthy();
    expect(
      screen.getByRole("combobox", { name: "Phiên bản Java" }),
    ).toHaveProperty("value", "17");
    expect(screen.getByText(/thường dùng Java 21/)).toBeTruthy();
  });
});
