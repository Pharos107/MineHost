import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Server } from "../../../api/client";
import { joinPath, pathSteps, type FileEntry } from "../../../api/files";
import { Toaster, toast } from "../../../ui";
import { FileEditorPage } from "./FileEditorPage";
import { FilesPage } from "./FilesPage";
import { canMoveInto } from "./MoveDialog";

// CodeMirror cần DOM thật để gõ; trang được thử với ô văn bản dự phòng khi trình sửa tải lỗi.
// CodeMirror có test riêng ở `ui/CodeEditor.test.tsx`.
vi.mock("../../../ui/codemirror", () => {
  throw new Error("không tải CodeMirror trong test trang");
});

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
      <Toaster />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
  toast.clear();
});

const entries: FileEntry[] = [
  { name: "world", kind: "dir" },
  { name: "server.properties", kind: "file", size: 1200, modified: "2026-10-27T01:00:00Z" },
  { name: "big.log", kind: "file", size: 5 * 1024 * 1024 },
  { name: "pack.zip", kind: "file", size: 2048 },
];

describe("FilesPage", () => {
  it("thư mục mở được, file văn bản/log mở trang xem sửa, file nén chỉ tải xuống", async () => {
    mockFetch((url) => (url.startsWith("/api/v1/servers/A/files?path=") ? Response.json(entries) : undefined));
    renderAt("/servers/A/files");
    const table = await screen.findByRole("table");
    expect(within(table).getByRole("link", { name: "world" }).getAttribute("href")).toBe("/servers/A/files?path=world");
    expect(within(table).getByRole("link", { name: "server.properties" }).getAttribute("href")).toBe("/servers/A/files/edit?path=server.properties");
    expect(within(table).getByRole("link", { name: "big.log" }).getAttribute("href")).toBe("/servers/A/files/edit?path=big.log");
    expect(within(table).queryByRole("link", { name: "pack.zip" })).toBeNull();
    expect(within(table).getByText("1,2 KB")).toBeTruthy();
  });

  it("tạo thư mục gửi đúng đường dẫn trong thư mục đang mở", async () => {
    const fetchMock = mockFetch((url, init) => {
      if (init?.method === "POST") return new Response(null, { status: 204 });
      if (url.startsWith("/api/v1/servers/A/files?path=")) return Response.json([]);
      return undefined;
    });
    renderAt("/servers/A/files?path=plugins");
    // Chờ hết trạng thái đang tải: thư mục trống thì nút nằm ở màn hình trống, không còn ở thanh trên bảng.
    await screen.findByText("Thư mục này trống");
    await userEvent.click(screen.getByRole("button", { name: "Thư mục mới" }));
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

describe("FilesPage nhiều mục", () => {
  it("chọn hai mục rồi xóa: hỏi một lần, xóa lần lượt, báo cả hai chỗ trong thùng rác", async () => {
    const fetchMock = mockFetch((url, init) => {
      if (init?.method === "DELETE") return Response.json({ trash_path: `/trash/${new URL(url, "http://x").searchParams.get("path")}` });
      if (url.startsWith("/api/v1/servers/A/files?path=")) return Response.json(entries);
      return undefined;
    });
    renderAt("/servers/A/files");
    await userEvent.click(await screen.findByRole("checkbox", { name: "Chọn world" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Chọn big.log" }));
    expect(screen.getByText("Đã chọn 2")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Xóa" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("Xóa 2 mục?")).toBeTruthy();
    await userEvent.click(within(dialog).getByRole("button", { name: "Xóa" }));
    expect(await screen.findByText("/trash/world")).toBeTruthy();
    expect(screen.getByText("/trash/big.log")).toBeTruthy();
    expect(fetchMock.mock.calls.filter(([, init]) => init?.method === "DELETE")).toHaveLength(2);
  });

  it("di chuyển vào thư mục con; không chọn được chính thư mục đang chuyển", async () => {
    const fetchMock = mockFetch((url, init) => {
      if (init?.method === "POST") return new Response(null, { status: 204 });
      if (url === "/api/v1/servers/A/files?path=world") return Response.json([{ name: "region", kind: "dir" }]);
      if (url.startsWith("/api/v1/servers/A/files?path=")) return Response.json(entries);
      return undefined;
    });
    renderAt("/servers/A/files");
    await userEvent.click(await screen.findByRole("button", { name: "Thao tác khác cho server.properties" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Di chuyển" }));
    const dialog = screen.getByRole("dialog");
    expect((within(dialog).getByRole("button", { name: "Chuyển vào đây" }) as HTMLButtonElement).disabled).toBe(true);
    await userEvent.click(await within(dialog).findByRole("button", { name: "world" }));
    await within(dialog).findByRole("button", { name: "region" });
    await userEvent.click(within(dialog).getByRole("button", { name: "Chuyển vào đây" }));
    const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
    expect(post?.[0]).toBe("/api/v1/servers/A/files/rename");
    expect(JSON.parse(String(post?.[1]?.body))).toEqual({ from: "server.properties", to: "world/server.properties" });
    expect(canMoveInto("world/region", ["world"])).toBe(false);
    expect(canMoveInto("worlds", ["world"])).toBe(true);
  });

  it("giải nén file zip vào thư mục mới mang tên file", async () => {
    const fetchMock = mockFetch((url, init) => {
      if (init?.method === "POST") return Response.json({ job_id: 9 }, { status: 202 });
      if (url.startsWith("/api/v1/servers/A/files?path=")) return Response.json([{ name: "Pack.zip", kind: "file", size: 10 }]);
      return undefined;
    });
    renderAt("/servers/A/files?path=plugins");
    await userEvent.click(await screen.findByRole("button", { name: "Thao tác khác cho Pack.zip" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Giải nén" }));
    await userEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Giải nén" }));
    const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
    expect(post?.[0]).toBe("/api/v1/servers/A/files/extract");
    expect(JSON.parse(String(post?.[1]?.body))).toEqual({ path: "plugins/Pack.zip", dest: "plugins/Pack" });
  });

  it("tìm cả thư mục con bật trong cài đặt của ô tìm; mặc định chỉ lọc thư mục đang mở", async () => {
    const fetchMock = mockFetch((url) => {
      if (url.startsWith("/api/v1/servers/A/files/search?path=plugins&q=config")) {
        return Response.json({ items: [{ path: "plugins/Essentials/config.yml", kind: "file", size: 4 }], truncated: false });
      }
      if (url.startsWith("/api/v1/servers/A/files?path=")) return Response.json([{ name: "Essentials", kind: "dir" }]);
      return undefined;
    });
    renderAt("/servers/A/files?path=plugins");
    const box = await screen.findByRole("searchbox", { name: "Tìm kiếm" });
    await userEvent.type(box, "config");
    expect(await screen.findByText("Không có kết quả cho “config”.")).toBeTruthy();
    expect(fetchMock.mock.calls.some(([url]) => String(url).includes("/files/search"))).toBe(false);

    await userEvent.click(screen.getByRole("button", { name: "Cài đặt tìm kiếm" }));
    const option = screen.getByRole("menuitemcheckbox", { name: "Tìm cả thư mục con" });
    expect(option.getAttribute("aria-checked")).toBe("false");
    await userEvent.click(option);
    expect(option.getAttribute("aria-checked")).toBe("true");
    const link = await screen.findByRole("link", { name: "Essentials/config.yml" });
    expect(link.getAttribute("href")).toBe("/servers/A/files/edit?path=plugins%2FEssentials%2Fconfig.yml");
    expect(screen.getByRole("searchbox", { name: "Tìm trong mọi thư mục con" })).toBeTruthy();
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
    expect(screen.getByRole("status").textContent).toBe("Chưa lưu");
    await userEvent.keyboard("{Control>}s{/Control}");
    const put = fetchMock.mock.calls.find(([, init]) => init?.method === "PUT");
    expect((put?.[1]?.headers as Record<string, string>)["If-Match"]).toBe('"abc"');
    expect(JSON.parse(String(put?.[1]?.body)).text).toBe("motd=Xin chào\npvp=false");
    expect(await screen.findByText("File đã bị đổi ở chỗ khác")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Chép thay đổi của tôi" })).toBeTruthy();
  });

  it("file nhị phân không mở được thì vẫn có link tải xuống", async () => {
    mockFetch((url) => (url.startsWith("/api/v1/servers/A/files/content") ? Response.json({ code: "FILE_NOT_TEXT", params: {} }, { status: 415 }) : undefined));
    renderAt("/servers/A/files/edit?path=world/region.mca");
    const link = await screen.findByRole("link", { name: "Tải xuống" });
    expect(link.getAttribute("href")).toBe("/api/v1/servers/A/files/download?path=world%2Fregion.mca");
  });

  it("file lớn tự chuyển sang xem trước phần cuối chỉ đọc, và file NBT giải mã sang JSON", async () => {
    mockFetch((url) => {
      if (url.startsWith("/api/v1/servers/A/files/content")) return Response.json({ code: "FILE_TOO_LARGE", params: {} }, { status: 413 });
      if (url.startsWith("/api/v1/servers/A/files/preview")) {
        return Response.json({ text: "dòng log cuối\n", total_bytes: 2 * 1024 * 1024, shown_bytes: 15, truncated: true });
      }
      if (url.startsWith("/api/v1/servers/A/files/nbt")) {
        return Response.json({ json: '{\n  "LevelName": "world"\n}', size: 512, compressed: true });
      }
      return undefined;
    });
    renderAt("/servers/A/files/edit?path=logs/latest.log");
    expect(await screen.findByText(/đang xem .* cuối ở chế độ chỉ đọc/)).toBeTruthy();
    expect((await screen.findByRole("textbox", { name: "Nội dung logs/latest.log" }) as HTMLTextAreaElement).value).toBe("dòng log cuối\n");

    renderAt("/servers/A/files/edit?path=world/level.dat");
    expect(await screen.findByText(/nén gzip/)).toBeTruthy();
    expect((await screen.findByRole("textbox", { name: "Nội dung world/level.dat" }) as HTMLTextAreaElement).value).toContain('"LevelName": "world"');
  });
});

describe("FilesPage nâng cấp", () => {
  it("nhân bản, tính dung lượng thư mục, tải từ URL và quản lý thùng rác", async () => {
    let trashed = [{ id: "20261118-120000", deleted_at: "2026-11-18T12:00:00Z", items: ["old.txt"], bytes: 42 }];
    const fetchMock = mockFetch((url, init) => {
      if (url === "/api/v1/servers/A/files/copy" && init?.method === "POST") return new Response(null, { status: 204 });
      if (url === "/api/v1/servers/A/files/size?path=world") return Response.json({ bytes: 4096, files: 3, dirs: 1, truncated: false });
      if (url === "/api/v1/servers/A/files/fetch" && init?.method === "POST") return Response.json({ job_id: 12 }, { status: 202 });
      if (url === "/api/v1/jobs/12") return Response.json({ id: 12, kind: "fetch_file", state: "succeeded" });
      if (url === "/api/v1/servers/A/files/trash" && (!init?.method || init.method === "GET")) return Response.json(trashed);
      if (url === "/api/v1/servers/A/files/trash/restore" && init?.method === "POST") {
        trashed = [];
        return Response.json(["old.txt"]);
      }
      if (url.startsWith("/api/v1/servers/A/files?path=")) return Response.json(entries);
      return undefined;
    });
    renderAt("/servers/A/files");

    // Tính dung lượng thư mục world
    await userEvent.click(await screen.findByRole("button", { name: "Thao tác khác cho world" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Tính dung lượng" }));
    expect(await screen.findByText("4 KB")).toBeTruthy();

    // Nhân bản server.properties
    await userEvent.click(screen.getByRole("button", { name: "Thao tác khác cho server.properties" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Nhân bản" }));
    await userEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Nhân bản" }));
    const copyCall = fetchMock.mock.calls.find(([u]) => u === "/api/v1/servers/A/files/copy");
    expect(JSON.parse(String(copyCall?.[1]?.body))).toEqual({ from: "server.properties", to: "server (bản sao).properties" });

    // Mở menu Tải lên -> Tải từ URL
    await userEvent.click(screen.getByRole("button", { name: "Tải lên" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Tải từ URL" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Đường dẫn (http hoặc https)" }), "https://example.com/plugin.jar");
    await userEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Tải về" }));
    expect(await screen.findByText("Đã tải file về server.")).toBeTruthy();

    // Mở thùng rác và khôi phục
    await userEvent.click(screen.getByRole("button", { name: "Thùng rác" }));
    await userEvent.click(await within(screen.getByRole("dialog")).findByRole("button", { name: "Khôi phục" }));
    expect(await screen.findByText("Đã khôi phục 1 mục.")).toBeTruthy();
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
