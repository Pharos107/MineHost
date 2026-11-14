import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Server } from "../../../api/client";
import type { InstalledContent, ServerContent } from "../../../api/content";
import { ContentPage } from "./ContentPage";

const stoppedFabric: Server = {
  id: "A",
  name: "Fabric Test",
  state: "stopped",
  loader: "fabric",
  content_kind: "mods",
  eula_accepted: true,
  restart_pending: false,
  slow_start: false,
  ignored_jvm_args: false,
  preflight_blocking: false,
};

const sampleFiles: InstalledContent[] = [
  {
    file_name: "lithium-0.14.2.jar",
    name: "Lithium",
    version: "0.14.2",
    enabled: true,
    size: 512000,
    source: "modrinth",
    manual: false,
    client_only: false,
    project_id: "gvQqBUqZ",
    version_id: "ver1",
    from_modpack: false,
    icon_url: null,
  },
  {
    file_name: "iris-1.8.0.jar",
    name: "Iris Shaders",
    version: "1.8.0",
    enabled: false,
    size: 1024000,
    source: "modrinth",
    manual: false,
    client_only: true,
    project_id: "YL57xq9U",
    version_id: "ver2",
    from_modpack: true,
    icon_url: "https://cdn.modrinth.com/iris.png",
  },
  {
    file_name: "custom-mod.jar",
    name: "custom-mod.jar",
    version: null,
    enabled: true,
    size: 4096,
    source: null,
    manual: true,
    client_only: false,
    project_id: null,
    version_id: null,
    from_modpack: false,
    icon_url: null,
  },
];

type Handler = (url: string, init?: RequestInit) => Response | Promise<Response> | undefined;

function mockFetch(handler: Handler) {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    const res = await handler(url, init);
    return res ?? Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

function renderPage(server: Server = stoppedFabric) {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <MemoryRouter initialEntries={["/servers/A/content"]}>
        <Routes>
          <Route path="/servers/:id" element={<Outlet context={server} />}>
            <Route path="content" element={<ContentPage />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("ContentPage", () => {
  it("hiện danh sách mod, nguồn, nhãn chỉ client và lọc theo tên", async () => {
    const content: ServerContent = { kind: "mods", files: sampleFiles, pending: [] };
    mockFetch((url) => (url === "/api/v1/servers/A/content" ? Response.json(content) : undefined));

    renderPage();
    const table = await screen.findByRole("table", { name: "Danh sách mod" });
    expect(within(table).getByText("Lithium")).toBeTruthy();
    expect(within(table).getByText("Iris Shaders")).toBeTruthy();
    expect(within(table).getByText("Chỉ chạy ở client")).toBeTruthy();
    expect(within(table).getByText("Modrinth")).toBeTruthy();
    expect(within(table).getByText("Modpack")).toBeTruthy();
    expect(within(table).getByText("Thêm tay")).toBeTruthy();
    expect(within(table).getByRole("switch", { name: "Bật Lithium" })).toHaveProperty("checked", true);
    expect(within(table).getByRole("switch", { name: "Bật Iris Shaders" })).toHaveProperty("checked", false);

    await userEvent.type(screen.getByRole("searchbox", { name: "Tìm kiếm" }), "Iris");
    expect(within(table).queryByText("Lithium")).toBeNull();
    expect(within(table).getByText("Iris Shaders")).toBeTruthy();
  });

  it("bật/tắt công tắc gọi PATCH và tải lại danh sách", async () => {
    let files = structuredClone(sampleFiles);
    const fetchMock = mockFetch((url, init) => {
      if (url === "/api/v1/servers/A/content") return Response.json({ kind: "mods", files, pending: [] });
      if (url === "/api/v1/servers/A/content/lithium-0.14.2.jar" && init?.method === "PATCH") {
        files = files.map((f) => (f.file_name === "lithium-0.14.2.jar" ? { ...f, enabled: false } : f));
        return new Response(null, { status: 204 });
      }
      return undefined;
    });

    renderPage();
    const toggle = await screen.findByRole("switch", { name: "Bật Lithium" });
    await userEvent.click(toggle);

    await waitFor(() => {
      const patch = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH");
      expect(patch?.[0]).toBe("/api/v1/servers/A/content/lithium-0.14.2.jar");
      expect(JSON.parse(String(patch?.[1]?.body))).toEqual({ enabled: false });
    });
    await waitFor(() => {
      expect(screen.getByRole("switch", { name: "Bật Lithium" })).toHaveProperty("checked", false);
    });
  });

  it("menu … có Mở trang mod (chỉ với Modrinth có project_id) và Gỡ có xác nhận vào thùng rác", async () => {
    let files = structuredClone(sampleFiles);
    const openSpy = vi.spyOn(window, "open").mockImplementation(() => null);
    const fetchMock = mockFetch((url, init) => {
      if (url === "/api/v1/servers/A/content") return Response.json({ kind: "mods", files, pending: [] });
      if (url === "/api/v1/servers/A/content/custom-mod.jar" && init?.method === "DELETE") {
        files = files.filter((f) => f.file_name !== "custom-mod.jar");
        return new Response(null, { status: 204 });
      }
      return undefined;
    });

    renderPage();
    await screen.findByText("Lithium");

    await userEvent.click(screen.getByRole("button", { name: "Thao tác khác cho Lithium" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Mở trang mod" }));
    expect(openSpy).toHaveBeenCalledWith("https://modrinth.com/project/gvQqBUqZ", "_blank", "noopener,noreferrer");

    await userEvent.click(screen.getByRole("button", { name: "Thao tác khác cho custom-mod.jar" }));
    expect(screen.queryByRole("menuitem", { name: "Mở trang mod" })).toBeNull();
    await userEvent.click(screen.getByRole("menuitem", { name: "Gỡ" }));

    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText(/thùng rác/)).toBeTruthy();
    await userEvent.click(within(dialog).getByRole("button", { name: "Gỡ" }));

    await waitFor(() => {
      const del = fetchMock.mock.calls.find(([, init]) => init?.method === "DELETE");
      expect(del?.[0]).toBe("/api/v1/servers/A/content/custom-mod.jar");
    });
    await waitFor(() => {
      expect(screen.queryByText("custom-mod.jar")).toBeNull();
    });
  });

  it("tải lên file .jar và báo lỗi đã dịch khi hỏng hoặc trùng", async () => {
    let files: InstalledContent[] = [];
    let failCode: string | null = null;
    const fetchMock = mockFetch((url, init) => {
      if (url === "/api/v1/servers/A/content") return Response.json({ kind: "mods", files, pending: [] });
      if (url === "/api/v1/servers/A/content/upload" && init?.method === "POST") {
        if (failCode) return Response.json({ code: failCode, params: {} }, { status: 409 });
        files = sampleFiles.slice(0, 1);
        return Response.json(files[0]);
      }
      return undefined;
    });

    renderPage();
    await screen.findByText("Chưa có mod nào");

    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    // Sai đuôi -> báo lỗi ngay, không gửi request.
    fireEvent.change(input, { target: { files: [new File(["bad"], "notes.txt")] } });
    expect(await screen.findByText("File không phải .jar hợp lệ hoặc bị hỏng.")).toBeTruthy();
    expect(fetchMock.mock.calls.some(([url]) => url.endsWith("/upload"))).toBe(false);

    // Trùng tên -> báo lỗi FILE_EXISTS.
    failCode = "FILE_EXISTS";
    fireEvent.change(input, { target: { files: [new File(["jar"], "lithium-0.14.2.jar")] } });
    expect(await screen.findByText("Đã có file hoặc thư mục trùng tên.")).toBeTruthy();

    // Hợp lệ -> hiện mod mới trong bảng.
    failCode = null;
    fireEvent.change(input, { target: { files: [new File(["jar"], "lithium-0.14.2.jar")] } });
    expect(await screen.findByText("Đã tải lên 1 file.")).toBeTruthy();
    expect(await screen.findByText("Lithium")).toBeTruthy();
  });

  it("khóa công tắc, tải lên và Gỡ khi server đang chạy", async () => {
    mockFetch((url) =>
      url === "/api/v1/servers/A/content" ? Response.json({ kind: "mods", files: sampleFiles, pending: [] }) : undefined,
    );
    renderPage({ ...stoppedFabric, state: "running" });

    expect(await screen.findByText(/Server đang chạy: tắt server trước/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Tải lên .jar" })).toHaveProperty("disabled", true);
    expect(await screen.findByRole("switch", { name: "Bật Lithium" })).toHaveProperty("disabled", true);

    await userEvent.click(screen.getByRole("button", { name: "Thao tác khác cho Lithium" }));
    expect(screen.getByRole("menuitem", { name: "Gỡ" })).toHaveProperty("disabled", true);
  });

  it("hiện trạng thái đang tải, lỗi và Vanilla không hỗ trợ", async () => {
    let resolveList!: (res: Response) => void;
    mockFetch((url) => {
      if (url === "/api/v1/servers/A/content") return new Promise((resolve) => (resolveList = resolve));
      return undefined;
    });
    renderPage();
    expect(screen.getByText("Đang tải…")).toBeTruthy();
    resolveList(Response.json({ code: "INTERNAL", params: {} }, { status: 500 }));
    expect(await screen.findByRole("alert")).toBeTruthy();

    renderPage({ ...stoppedFabric, loader: "vanilla", content_kind: null });
    expect(screen.getByText("Loại server này không hỗ trợ cài mod hoặc plugin.")).toBeTruthy();
  });
});
