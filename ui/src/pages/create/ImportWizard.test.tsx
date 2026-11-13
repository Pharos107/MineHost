import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Detected } from "../../api/client";
import { CreateServerPage } from "./CreateServerPage";
import { importReady, importRequest } from "./importForm";

vi.mock("../../api/ws", () => ({
  live: { subscribe: () => () => {} },
}));

function mockApi(detected: Detected) {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    if (url === "/api/v1/servers/import/inspect") return Response.json(detected);
    if (url === "/api/v1/servers/import") return Response.json({ server_id: "01JB", job_id: 9 }, { status: 202 });
    if (url.startsWith("/api/v1/catalog/")) return Response.json([]);
    if (url === "/api/v1/jobs/9") return Response.json({ id: 9, kind: "import_server", state: "running", started_at: "" });
    void init;
    return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

function renderWizard() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/new"]}>
        <Routes>
          <Route path="/new" element={<CreateServerPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

async function openFolder(user: ReturnType<typeof userEvent.setup>, path: string) {
  await user.click(screen.getByRole("radio", { name: /Nhập thư mục có sẵn/ }));
  await user.click(screen.getByRole("button", { name: "Tiếp" }));
  await user.type(screen.getByRole("textbox", { name: "Thư mục server" }), path);
  await user.click(screen.getByRole("button", { name: "Kiểm tra thư mục" }));
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("importForm", () => {
  const found: Detected = {
    loader: "paper",
    loader_version: null,
    minecraft: "1.21.4",
    jar: "paper.jar",
    has_mods: false,
    ignored_jvm_args: false,
  };
  const choice = { path: " /srv/mc ", loader: "" as const, minecraft: "", name: " Cũ " };

  it("chỉ gửi loại và phiên bản khi người dùng chọn tay", () => {
    expect(importReady(choice, found)).toBe(true);
    expect(importRequest(choice, found)).toEqual({ path: "/srv/mc", name: "Cũ", loader: null, minecraft: null });
    const unknown = { ...found, loader: null, minecraft: null };
    expect(importReady(choice, unknown)).toBe(false);
    const picked = { ...choice, loader: "vanilla" as const, minecraft: "1.20.4" };
    expect(importRequest(picked, unknown)).toMatchObject({ loader: "vanilla", minecraft: "1.20.4" });
  });

  it("server thường có mod hoặc thiếu jar thì không đi tiếp; mod loader với mods và args_file thì được", () => {
    expect(importReady(choice, { ...found, has_mods: true })).toBe(false);
    expect(importReady(choice, { ...found, jar: null })).toBe(false);
    expect(importReady(choice, undefined)).toBe(false);
    expect(
      importReady(choice, {
        ...found,
        loader: "neoforge",
        loader_version: "21.4.140",
        jar: null,
        has_mods: true,
      }),
    ).toBe(true);
  });
});

describe("ImportWizard", () => {
  it("không nhận ra loại thì cho chọn tay rồi gửi yêu cầu nhập", async () => {
    const fetchMock = mockApi({
      loader: null,
      loader_version: null,
      minecraft: "1.20.4",
      jar: "custom.jar",
      has_mods: false,
      ignored_jvm_args: false,
    });
    const user = userEvent.setup();
    renderWizard();
    await openFolder(user, "/srv/mc");

    expect(await screen.findByText("custom.jar")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Tiếp" })).toHaveProperty("disabled", true);
    await user.selectOptions(screen.getByRole("combobox", { name: "Chọn loại server" }), "vanilla");
    await user.click(screen.getByRole("button", { name: "Tiếp" }));
    await user.click(screen.getByRole("button", { name: "Nhập server" }));

    const post = fetchMock.mock.calls.find(([url]) => url === "/api/v1/servers/import");
    expect(JSON.parse(String(post?.[1]?.body))).toEqual({ path: "/srv/mc", name: "Server của tôi", loader: "vanilla", minecraft: null });
  });

  it("thư mục Paper có mod thì báo chưa hỗ trợ", async () => {
    mockApi({
      loader: "paper",
      loader_version: null,
      minecraft: "1.21.4",
      jar: "paper.jar",
      has_mods: true,
      ignored_jvm_args: false,
    });
    const user = userEvent.setup();
    renderWizard();
    await openFolder(user, "/srv/modded");

    expect(await screen.findByText(/chưa hỗ trợ mod/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Tiếp" })).toHaveProperty("disabled", true);
  });

  it("thư mục NeoForge có mod và user_jvm_args.txt vẫn đi tiếp và hiện cảnh báo", async () => {
    mockApi({
      loader: "neoforge",
      loader_version: "21.4.140",
      minecraft: "1.21.4",
      jar: null,
      has_mods: true,
      ignored_jvm_args: true,
    });
    const user = userEvent.setup();
    renderWizard();
    await openFolder(user, "/srv/neoforge");

    expect(await screen.findByText("21.4.140")).toBeTruthy();
    expect(screen.getByText(/user_jvm_args\.txt/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Tiếp" })).toHaveProperty("disabled", false);
  });
});
