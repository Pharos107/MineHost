import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Server } from "../../../api/client";
import { joinPath, pathSteps, type FileEntry } from "../../../api/files";
import { FileEditorPage } from "./FileEditorPage";
import { FilesPage } from "./FilesPage";

const server: Server = { id: "A", name: "Survival", state: "running", eula_accepted: true, restart_pending: false, slow_start: false, ignored_jvm_args: false, preflight_blocking: false };

type Handler = (url: string, init?: RequestInit) => Response | undefined;

function mockFetch(handler: Handler) {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => handler(url, init) ?? Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 }));
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

function Layout() {
  return <Outlet context={server} />;
}

function renderAt(path: string) {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/servers/:id" element={<Layout />}>
            <Route path="files" element={<FilesPage />} />
            <Route path="files/edit" element={<FileEditorPage />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

const entries: FileEntry[] = [
  { name: "world", kind: "dir" },
  { name: "server.properties", kind: "file", size: 1200, modified: "2026-10-27T01:00:00Z" },
  { name: "big.log", kind: "file", size: 5 * 1024 * 1024 },
];

describe("FilesPage", () => {
  it("thư mục mở được, file nhỏ mở trình sửa, file lớn chỉ tải xuống", async () => {
    mockFetch((url) => (url.startsWith("/api/v1/servers/A/files?path=") ? Response.json(entries) : undefined));
    renderAt("/servers/A/files");
    const table = await screen.findByRole("table");
    expect(within(table).getByRole("link", { name: "world" }).getAttribute("href")).toBe("/servers/A/files?path=world");
    expect(within(table).getByRole("link", { name: "server.properties" }).getAttribute("href")).toBe("/servers/A/files/edit?path=server.properties");
    expect(within(table).queryByRole("link", { name: "big.log" })).toBeNull();
    expect(within(table).getByText("1,2 KB")).toBeTruthy();
  });

  it("tạo thư mục gửi đúng đường dẫn trong thư mục đang mở", async () => {
    const fetchMock = mockFetch((url, init) => {
      if (init?.method === "POST") return new Response(null, { status: 204 });
      if (url.startsWith("/api/v1/servers/A/files?path=")) return Response.json([]);
      return undefined;
    });
    renderAt("/servers/A/files?path=plugins");
    await userEvent.click(await screen.findByRole("button", { name: "Thư mục mới" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Tên thư mục" }), "Essentials");
    await userEvent.click(screen.getByRole("button", { name: "Tạo" }));
    const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
    expect(post?.[0]).toBe("/api/v1/servers/A/files/mkdir");
    expect(JSON.parse(String(post?.[1]?.body))).toEqual({ path: "plugins/Essentials" });
  });

  it("xóa thì hỏi lại rồi báo chỗ trong thùng rác", async () => {
    mockFetch((url, init) => {
      if (init?.method === "DELETE") return Response.json({ trash_path: "/data/trash/A/files/x/world" });
      if (url.startsWith("/api/v1/servers/A/files?path=")) return Response.json(entries);
      return undefined;
    });
    renderAt("/servers/A/files");
    await userEvent.click(await screen.findByRole("button", { name: "Thao tác khác cho world" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Xóa" }));
    await userEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Xóa" }));
    expect(await screen.findByText("/data/trash/A/files/x/world")).toBeTruthy();
  });
});

describe("FileEditorPage", () => {
  it("Ctrl+S lưu kèm If-Match; file bị đổi ở chỗ khác thì cho tải lại hoặc chép", async () => {
    const fetchMock = mockFetch((url, init) => {
      if (init?.method === "PUT") return Response.json({ code: "FILE_CHANGED", params: {} }, { status: 412 });
      if (url.startsWith("/api/v1/servers/A/files/content")) return Response.json({ text: "motd=Xin chào\n", etag: "abc", size: 15 });
      return undefined;
    });
    renderAt("/servers/A/files/edit?path=server.properties");
    const editor = await screen.findByRole("textbox", { name: "Nội dung server.properties" });
    expect((editor as HTMLTextAreaElement).value).toBe("motd=Xin chào\n");
    await userEvent.type(editor, "pvp=false");
    expect(screen.getByText("Có thay đổi chưa lưu.")).toBeTruthy();
    await userEvent.keyboard("{Control>}s{/Control}");
    const put = fetchMock.mock.calls.find(([, init]) => init?.method === "PUT");
    expect((put?.[1]?.headers as Record<string, string>)["If-Match"]).toBe('"abc"');
    expect(JSON.parse(String(put?.[1]?.body)).text).toBe("motd=Xin chào\npvp=false");
    expect(await screen.findByText("File đã bị đổi ở chỗ khác")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Chép thay đổi của tôi" })).toBeTruthy();
  });

  it("file nhị phân không mở được thì vẫn có link tải xuống", async () => {
    mockFetch((url) => (url.startsWith("/api/v1/servers/A/files/content") ? Response.json({ code: "FILE_NOT_TEXT", params: {} }, { status: 415 }) : undefined));
    renderAt("/servers/A/files/edit?path=world/level.dat");
    const link = await screen.findByRole("link", { name: "Tải xuống" });
    expect(link.getAttribute("href")).toBe("/api/v1/servers/A/files/download?path=world%2Flevel.dat");
  });
});

describe("đường dẫn", () => {
  it("ghép và tách đường dẫn cho breadcrumb", () => {
    expect(joinPath("", "a")).toBe("a");
    expect(joinPath("a/b", "c")).toBe("a/b/c");
    expect(pathSteps("world/region")).toEqual([
      { name: "world", path: "world" },
      { name: "region", path: "world/region" },
    ]);
    expect(pathSteps("")).toEqual([]);
  });
});
