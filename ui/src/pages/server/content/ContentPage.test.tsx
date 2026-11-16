import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Server } from "../../../api/client";
import type { InstalledContent, PendingContent, ServerContent } from "../../../api/content";
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
    needs_restart: false,
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
    needs_restart: false,
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
    needs_restart: false,
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

function pageTree(server: Server, client: QueryClient) {
  return (
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/servers/A/content"]}>
        <Routes>
          <Route path="/servers/:id" element={<Outlet context={server} />}>
            <Route path="content" element={<ContentPage />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  );
}

function renderPage(server: Server = stoppedFabric) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const view = render(pageTree(server, client));
  return { rerender: (next: Server) => view.rerender(pageTree(next, client)) };
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

  it("server đang chạy vẫn thêm được mod, chỉ khóa bật/tắt và Gỡ, báo mod chờ khởi động lại", async () => {
    const files = sampleFiles.map((f, i) => (i === 0 ? { ...f, needs_restart: true } : f));
    const fetchMock = mockFetch((url, init) => {
      if (url === "/api/v1/servers/A/content") return Response.json({ kind: "mods", files, pending: [] });
      if (url === "/api/v1/servers/A/actions/restart" && init?.method === "POST") return new Response(null, { status: 204 });
      return undefined;
    });
    renderPage({ ...stoppedFabric, state: "running" });

    const table = await screen.findByRole("table", { name: "Danh sách mod" });
    expect(screen.queryByText(/Server đang chạy/)).toBeNull();
    expect(screen.getByRole("button", { name: "Thêm mod" })).toHaveProperty("disabled", false);
    expect(screen.getByRole("button", { name: "Tải lên .jar" })).toHaveProperty("disabled", false);
    const toggle = within(table).getByRole("switch", { name: "Bật Lithium" });
    expect(toggle).toHaveProperty("disabled", true);
    expect(toggle.closest("label")?.getAttribute("title")).toBe("Tắt server để bật, tắt hoặc gỡ mod");
    expect(within(table).getByText("Chờ khởi động lại")).toBeTruthy();

    expect(screen.getByText("1 mod mới sẽ chạy sau khi khởi động lại server.")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Khởi động lại" }));
    await waitFor(() =>
      expect(fetchMock.mock.calls.some(([u, i]) => u === "/api/v1/servers/A/actions/restart" && i?.method === "POST")).toBe(true),
    );

    await userEvent.click(screen.getByRole("button", { name: "Thao tác khác cho Lithium" }));
    expect(screen.getByRole("menuitem", { name: "Gỡ" })).toHaveProperty("disabled", true);
  });

  it("server đổi trạng thái thì tải lại danh sách, nhãn chờ khởi động lại mất", async () => {
    let waiting = true;
    mockFetch((url) =>
      url === "/api/v1/servers/A/content"
        ? Response.json({ kind: "mods", files: [{ ...sampleFiles[0], needs_restart: waiting }], pending: [] })
        : undefined,
    );
    const view = renderPage({ ...stoppedFabric, state: "running" });
    expect(await screen.findByText("Chờ khởi động lại")).toBeTruthy();
    waiting = false;
    view.rerender({ ...stoppedFabric, state: "starting" });
    await waitFor(() => expect(screen.queryByText("Chờ khởi động lại")).toBeNull());
  });

  it("server đang cài thì khóa cả Thêm mod và Tải lên", async () => {
    mockFetch((url) =>
      url === "/api/v1/servers/A/content" ? Response.json({ kind: "mods", files: sampleFiles, pending: [] }) : undefined,
    );
    renderPage({ ...stoppedFabric, state: "installing" });
    await screen.findByRole("table", { name: "Danh sách mod" });
    expect(screen.getByRole("button", { name: "Thêm mod" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "Tải lên .jar" })).toHaveProperty("disabled", true);
  });

  it("kéo thả nhiều file .jar vào bảng thì tải lên từng file", async () => {
    let uploads = 0;
    mockFetch((url, init) => {
      if (url === "/api/v1/servers/A/content") return Response.json({ kind: "mods", files: sampleFiles, pending: [] });
      if (url === "/api/v1/servers/A/content/upload" && init?.method === "POST") {
        uploads += 1;
        return Response.json(sampleFiles[0]);
      }
      return undefined;
    });
    renderPage();
    const table = await screen.findByRole("table", { name: "Danh sách mod" });
    const files = [new File(["a"], "a.jar"), new File(["b"], "b.jar")];
    const dataTransfer = { files, types: ["Files"] };
    fireEvent.dragEnter(table, { dataTransfer });
    expect(screen.getByText(/Thả file .jar để thêm vào server/)).toBeTruthy();
    fireEvent.drop(table, { dataTransfer });
    expect(await screen.findByText("Đã tải lên 2 file.")).toBeTruthy();
    expect(uploads).toBe(2);
    expect(screen.queryByText(/Thả file .jar để thêm vào server/)).toBeNull();
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

  it("hiện hộp chờ CurseForge, báo UPLOAD_HASH_MISMATCH, chuyển Đã có và bật không cần chờ", async () => {
    let pending: PendingContent[] = [
      {
        path: "mods/cf-restricted-1.0.0.jar",
        name: "Restricted Mod",
        file_name: "cf-restricted-1.0.0.jar",
        project_id: 999001,
        file_id: 5800999,
        sha1: "a".repeat(40),
        page_url: "https://www.curseforge.com/minecraft/mc-mods/restricted/download/5800999",
      },
    ];
    let files: InstalledContent[] = [];
    const fetchMock = mockFetch((url, init) => {
      if (url === "/api/v1/servers/A/content") {
        return Response.json({ kind: "mods", files, pending });
      }
      if (url === "/api/v1/servers/A/content/pending/upload" && init?.method === "POST") {
        const installed: InstalledContent = {
          file_name: "cf-restricted-1.0.0.jar",
          name: "Restricted Mod",
          version: "1.0.0",
          enabled: true,
          size: 256,
          source: "curseforge",
          manual: false,
          client_only: false,
          project_id: "999001",
          version_id: "5800999",
          from_modpack: false,
          icon_url: null,
          needs_restart: false,
        };
        pending = [];
        files = [installed];
        return Response.json({
          results: [
            {
              file_name: "wrong.jar",
              ok: false,
              installed: null,
              error: { code: "UPLOAD_HASH_MISMATCH", params: { expected: "cf-restricted-1.0.0.jar" } },
            },
            {
              file_name: "downloaded.jar",
              ok: true,
              installed,
              error: null,
            },
          ],
        });
      }
      if (url === "/api/v1/servers/A/actions/start" && init?.method === "POST") {
        return new Response(null, { status: 204 });
      }
      return undefined;
    });

    renderPage();
    expect(await screen.findByText("Restricted Mod")).toBeTruthy();
    expect(screen.getByText("Chờ")).toBeTruthy();
    const link = screen.getByRole("link", { name: "Mở trang tải" });
    expect(link.getAttribute("href")).toContain("/download/5800999");

    // Bấm "Bật mà không có các mod này" trước khi thả file -> gửi ignore_preflight=true.
    await userEvent.click(screen.getByRole("button", { name: "Bật mà không có các mod này" }));
    await waitFor(() => {
      const startCall = fetchMock.mock.calls.find(([url]) => url === "/api/v1/servers/A/actions/start");
      expect(startCall).toBeTruthy();
      expect(JSON.parse(String(startCall?.[1]?.body))).toEqual({ ignore_preflight: true });
    });

    // Kéo thả nhiều file: một sai (UPLOAD_HASH_MISMATCH kèm tên đúng), một đúng (chuyển Đã có).
    const input = screen.getByLabelText("Chọn file CurseForge") as HTMLInputElement;
    fireEvent.change(input, {
      target: { files: [new File(["w"], "wrong.jar"), new File(["r"], "downloaded.jar")] },
    });

    expect(await screen.findByText(/wrong\.jar:.*cf-restricted-1\.0\.0\.jar/)).toBeTruthy();
    expect(await screen.findByText("Đã có")).toBeTruthy();
    expect(screen.getByText(/Đã có đủ các file CurseForge/)).toBeTruthy();
    // Hết mục chờ sau khi tải lại danh sách, lỗi của file sai vẫn phải còn.
    expect(screen.getByText(/wrong\.jar:.*cf-restricted-1\.0\.0\.jar/)).toBeTruthy();
  });
});
