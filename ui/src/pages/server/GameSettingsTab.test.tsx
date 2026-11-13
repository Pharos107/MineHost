import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Server } from "../../api/client";
import { GameSettingsTab } from "./GameSettingsTab";

const server: Server = { id: "01JB", name: "Survival", state: "running", eula_accepted: true, restart_pending: false, slow_start: false, ignored_jvm_args: false };
const properties = [
  { key: "server-port", value: "25565" },
  { key: "difficulty", value: "easy" },
  { key: "view-distance", value: "10" },
];

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("GameSettingsTab", () => {
  it("lưu cả file theo thứ tự cũ, ẩn khóa cổng do MineHost quản lý", async () => {
    const fetchMock = vi.fn(async (_url: string, init?: RequestInit) =>
      init?.method === "PUT" ? new Response(null, { status: 204 }) : Response.json({ properties }),
    );
    vi.stubGlobal("fetch", fetchMock);
    render(
      <QueryClientProvider client={new QueryClient()}>
        <GameSettingsTab server={server} />
      </QueryClientProvider>,
    );

    expect(await screen.findByText("Có hiệu lực sau khi khởi động lại server.")).toBeTruthy();
    expect(screen.queryByRole("textbox", { name: "server-port" })).toBeNull();
    expect(screen.getByRole("button", { name: "Lưu" })).toHaveProperty("disabled", true);

    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Độ khó" }), "hard");
    await userEvent.click(screen.getByRole("button", { name: "Lưu" }));

    await vi.waitFor(() => expect(fetchMock.mock.calls.some(([, init]) => init?.method === "PUT")).toBe(true));
    const put = fetchMock.mock.calls.find(([, init]) => init?.method === "PUT");
    expect(JSON.parse(String(put?.[1]?.body)).properties).toEqual([
      { key: "server-port", value: "25565" },
      { key: "difficulty", value: "hard" },
      { key: "view-distance", value: "10" },
    ]);
  });
});
