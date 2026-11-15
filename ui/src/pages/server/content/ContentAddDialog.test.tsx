import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Job, Server } from "../../../api/client";
import type { ContentPlan, ContentVersion, InstalledContent, SearchHit } from "../../../api/content";
import type { LiveMessage } from "../../../api/ws";
import { ContentPage } from "./ContentPage";

const handlers = new Map<string, (message: LiveMessage) => void>();

vi.mock("../../../api/ws", () => ({
  live: {
    subscribe: (topic: string, handler: (message: LiveMessage) => void) => {
      handlers.set(topic, handler);
      return () => handlers.delete(topic);
    },
  },
}));

const stoppedFabric: Server = {
  id: "A",
  name: "Fabric Test",
  state: "stopped",
  loader: "fabric",
  minecraft: "1.21.4",
  content_kind: "mods",
  eula_accepted: true,
  restart_pending: false,
  slow_start: false,
  ignored_jvm_args: false,
  preflight_blocking: false,
};

const stoppedPaper: Server = {
  ...stoppedFabric,
  id: "P",
  name: "Paper Test",
  loader: "paper",
  content_kind: "plugins",
};

const lithiumHit: SearchHit = {
  source: "modrinth",
  project_id: "lithium",
  slug: "lithium",
  kind: "mod",
  title: "Lithium",
  description: "Tối ưu hiệu năng server Fabric.",
  icon_url: null,
  downloads: 500_000,
  categories: ["fabric", "optimization"],
  game_versions: ["1.21.4"],
  server_side: "optional",
};

const fabricApiHit: SearchHit = {
  ...lithiumHit,
  project_id: "fabric-api",
  slug: "fabric-api",
  title: "Fabric API",
  description: "Thư viện nền cho mod Fabric.",
  downloads: 1_000_000,
};

const lithiumVersions: ContentVersion[] = [
  {
    id: "lithium-0.14.2",
    project_id: "lithium",
    name: "v0.14.2",
    version_number: "0.14.2",
    version_type: "release",
    game_versions: ["1.21.4"],
    loaders: ["fabric"],
    date_published: "2024-12-12T00:00:00Z",
    file_name: "lithium-0.14.2.jar",
    file_size: 650_000,
    manual_download: false,
    download_page_url: null,
  },
  {
    id: "lithium-0.14.0",
    project_id: "lithium",
    name: "v0.14.0",
    version_number: "0.14.0",
    version_type: "beta",
    game_versions: ["1.21.4"],
    loaders: ["fabric"],
    date_published: "2024-12-05T00:00:00Z",
    file_name: "lithium-0.14.0.jar",
    file_size: 620_000,
    manual_download: false,
    download_page_url: null,
  },
];

const samplePlan: ContentPlan = {
  kind: "mods",
  items: [
    {
      project_id: "lithium",
      project_name: "Lithium",
      version_id: "lithium-0.14.2",
      version_number: "0.14.2",
      file_name: "lithium-0.14.2.jar",
      file_size: 650_000,
      client_only: true,
      replaces_existing: true,
      is_dependency: false,
      manual_download: false,
      download_page_url: null,
    },
    {
      project_id: "fabric-api",
      project_name: "Fabric API",
      version_id: "fabric-api-0.110.0",
      version_number: "0.110.0",
      file_name: "fabric-api-0.110.0.jar",
      file_size: 2_100_000,
      client_only: false,
      replaces_existing: false,
      is_dependency: true,
      manual_download: false,
      download_page_url: null,
    },
  ],
};

const runningJob: Job = {
  id: 12,
  kind: "install_content",
  state: "running",
  server_id: "A",
  started_at: "2026-11-15T00:00:00Z",
};

type Handler = (url: string, init?: RequestInit) => Response | Promise<Response> | undefined;

function mockFetch(handler: Handler) {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    const res = await handler(url, init);
    return res ?? Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

function renderPage(server: Server = stoppedFabric, initialPath = `/servers/${server.id}/content`) {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <MemoryRouter initialEntries={[initialPath]}>
        <Routes>
          <Route path="/servers/:id" element={<Outlet context={server} />}>
            <Route path="content" element={<ContentPage />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function pushJob(job: Partial<Job>) {
  act(() => handlers.get("job.12")?.({ topic: "job.12", seq: 1, data: { ...runningJob, ...job } }));
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  handlers.clear();
});

describe("ContentAddDialog", () => {
  it("tìm mod, xem thêm trang, chọn bản, xem dependency và cài đặt tới khi xong", async () => {
    let installed: InstalledContent[] = [];
    const fetchMock = mockFetch((url, init) => {
      if (url === "/api/v1/settings/curseforge") {
        return Response.json({ available: false, source: "none" });
      }
      if (url === "/api/v1/servers/A/content" && (!init?.method || init.method === "GET")) {
        return Response.json({ kind: "mods", files: installed, pending: [] });
      }
      if (url.startsWith("/api/v1/content/search")) {
        const params = new URL(url, "http://localhost").searchParams;
        const page = Number(params.get("page") ?? "1");
        const q = params.get("q") ?? "";
        if (q === "Lithium") {
          return Response.json({ hits: [lithiumHit], page: 1, page_size: 20, total: 1 });
        }
        if (page === 1) {
          return Response.json({ hits: [fabricApiHit], page: 1, page_size: 1, total: 2 });
        }
        return Response.json({ hits: [lithiumHit], page: 2, page_size: 1, total: 2 });
      }
      if (url === "/api/v1/content/modrinth/projects/lithium/versions?server_id=A") {
        return Response.json(lithiumVersions);
      }
      if (url === "/api/v1/servers/A/content/plan" && init?.method === "POST") {
        const body = JSON.parse(String(init.body)) as { version_id?: string };
        return Response.json({
          ...samplePlan,
          items: samplePlan.items.map((item, i) =>
            i === 0 && body.version_id === "lithium-0.14.0" ? { ...item, version_id: "lithium-0.14.0", version_number: "0.14.0" } : item,
          ),
        });
      }
      if (url === "/api/v1/servers/A/content" && init?.method === "POST") {
        return Response.json({ job_id: 12 }, { status: 202 });
      }
      if (url === "/api/v1/jobs/12") {
        return Response.json(runningJob);
      }
      return undefined;
    });

    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("button", { name: "Thêm" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByRole("heading", { name: "Thêm mod" })).toBeTruthy();

    // Trang 1 hiện Fabric API; bấm Xem thêm tải trang 2 có Lithium.
    expect(await within(dialog).findByText("Fabric API")).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "Xem thêm" }));
    expect(await within(dialog).findByText("Lithium")).toBeTruthy();

    // Gõ lọc tìm Lithium, chọn mod -> hiện bản mới nhất (0.14.2) và danh sách dependency.
    const searchBox = within(dialog).getByRole("searchbox", { name: "Tìm mod trên Modrinth…" });
    await user.type(searchBox, "Lithium{Enter}");
    await user.click(await within(dialog).findByRole("button", { name: /Lithium/ }));

    const versionSelect = await within(dialog).findByRole("combobox", { name: "Phiên bản" });
    expect(versionSelect).toHaveProperty("value", "lithium-0.14.2");
    expect(await within(dialog).findByText("Danh sách sẽ cài")).toBeTruthy();
    expect(within(dialog).getByText("Mod sẽ cài")).toBeTruthy();
    expect(within(dialog).getByText("Dependency cài kèm (1)")).toBeTruthy();
    expect(within(dialog).getByText("Fabric API")).toBeTruthy();
    expect(within(dialog).getByText("Chỉ chạy ở client")).toBeTruthy();
    expect(within(dialog).getByText("Thay bản cũ")).toBeTruthy();

    // Đổi sang bản cũ hơn -> gọi plan với version_id mới.
    await user.selectOptions(versionSelect, "lithium-0.14.0");
    await waitFor(() => expect(within(dialog).getByText("0.14.0")).toBeTruthy());

    // Bấm Cài -> gửi đúng body, hiện tiến độ Job, xong thì đóng và làm mới danh sách.
    await user.click(within(dialog).getByRole("button", { name: "Cài" }));
    await waitFor(() => {
      const post = fetchMock.mock.calls.find(([u, i]) => u === "/api/v1/servers/A/content" && i?.method === "POST");
      expect(JSON.parse(String(post?.[1]?.body))).toEqual({
        source: "modrinth",
        project_id: "lithium",
        version_id: "lithium-0.14.0",
      });
    });

    expect(await within(dialog).findByText("Đang bắt đầu…")).toBeTruthy();
    pushJob({ progress: { step: { kind: "download_content", name: "Lithium" } as never, step_index: 1, step_count: 2 } });
    expect(await within(dialog).findByText("Tải Lithium")).toBeTruthy();

    installed = [
      {
        file_name: "lithium-0.14.0.jar",
        name: "Lithium",
        version: "0.14.0",
        enabled: true,
        size: 620_000,
        source: "modrinth",
        manual: false,
        client_only: false,
        project_id: "lithium",
        version_id: "lithium-0.14.0",
        from_modpack: false,
        icon_url: null,
      },
    ];
    pushJob({ state: "succeeded" });

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    const table = await screen.findByRole("table", { name: "Danh sách mod" });
    expect(within(table).getByText("Lithium")).toBeTruthy();
  });

  it("gửi kind=plugin với server Paper và dịch các mã lỗi API", async () => {
    let searchError: string | null = "UPSTREAM_RATE_LIMITED";
    let planError: string | null = "CONTENT_INCOMPATIBLE";
    let installError: string | null = "SERVER_BUSY";

    const fetchMock = mockFetch((url, init) => {
      if (url === "/api/v1/servers/P/content" && (!init?.method || init.method === "GET")) {
        return Response.json({ kind: "plugins", files: [], pending: [] });
      }
      if (url.startsWith("/api/v1/content/search")) {
        if (searchError === "UPSTREAM_RATE_LIMITED") {
          return Response.json({ code: "UPSTREAM_RATE_LIMITED", params: { retry_after_s: 30 } }, { status: 429 });
        }
        if (searchError === "NETWORK_UNAVAILABLE") {
          return Response.json({ code: "NETWORK_UNAVAILABLE", params: {} }, { status: 503 });
        }
        return Response.json({ hits: [lithiumHit], page: 1, page_size: 20, total: 1 });
      }
      if (url === "/api/v1/content/modrinth/projects/lithium/versions?server_id=P") {
        return Response.json(lithiumVersions);
      }
      if (url === "/api/v1/servers/P/content/plan" && init?.method === "POST") {
        if (planError) return Response.json({ code: planError, params: {} }, { status: 409 });
        return Response.json(samplePlan);
      }
      if (url === "/api/v1/servers/P/content" && init?.method === "POST") {
        return Response.json({ code: installError ?? "INTERNAL", params: {} }, { status: 409 });
      }
      return undefined;
    });

    const user = userEvent.setup();
    renderPage(stoppedPaper, "/servers/P/content?add=1");
    const dialog = await screen.findByRole("dialog");

    // Kiểm tra query params gửi kind=plugin và server_id=P.
    await waitFor(() => {
      const searchCall = fetchMock.mock.calls.find(([u]) => u.startsWith("/api/v1/content/search"));
      expect(searchCall?.[0]).toContain("kind=plugin");
      expect(searchCall?.[0]).toContain("server_id=P");
    });

    expect(await within(dialog).findByText(/Thử lại sau 30 giây/)).toBeTruthy();

    searchError = "NETWORK_UNAVAILABLE";
    await user.click(within(dialog).getByRole("button", { name: "Thử lại" }));
    expect(await within(dialog).findByText(/Không kết nối được Internet/)).toBeTruthy();

    searchError = null;
    await user.click(within(dialog).getByRole("button", { name: "Thử lại" }));
    await user.click(await within(dialog).findByRole("button", { name: /Lithium/ }));

    expect(await within(dialog).findByText(/không có bản cho loại server/)).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "Cài" })).toHaveProperty("disabled", true);

    // Khi plan hợp lệ nhưng install gặp SERVER_BUSY -> hiện lỗi đã dịch.
    planError = null;
    await user.click(within(dialog).getByRole("button", { name: "Quay lại" }));
    await user.click(await within(dialog).findByRole("button", { name: /Lithium/ }));
    await within(dialog).findByText("Danh sách sẽ cài");
    installError = "SERVER_BUSY";
    await user.click(within(dialog).getByRole("button", { name: "Cài" }));
    expect(await within(dialog).findByText(/Server đang bận/)).toBeTruthy();
  });

  it("làm mờ nút CurseForge kèm lý do khi chưa cấu hình API key", async () => {
    mockFetch((url) => {
      if (url === "/api/v1/servers/A/content") return Response.json({ kind: "mods", files: [], pending: [] });
      if (url === "/api/v1/settings/curseforge") return Response.json({ available: false, source: "none" });
      if (url.startsWith("/api/v1/content/search")) return Response.json({ hits: [lithiumHit], page: 1, page_size: 20, total: 1 });
      return undefined;
    });

    renderPage(stoppedFabric, "/servers/A/content?add=1");
    const dialog = await screen.findByRole("dialog");

    expect(await within(dialog).findByText(/chưa cấu hình API key/)).toBeTruthy();
    const cfRadio = within(dialog).getByRole("radio", { name: "CurseForge" });
    expect(cfRadio).toHaveProperty("disabled", true);
    expect(within(dialog).getByRole("radio", { name: "Modrinth" })).toHaveProperty("disabled", false);
  });

  it("chuyển sang CurseForge khi có key, hiện file tải tay kèm link và gửi đúng source", async () => {
    const cfHit: SearchHit = {
      source: "curseforge",
      project_id: "999001",
      slug: "restricted-mod",
      kind: "mod",
      title: "Restricted Mod",
      description: "Mod chặn phân phối qua API.",
      icon_url: null,
      downloads: 42_000,
      categories: [],
      game_versions: [],
      server_side: null,
    };
    const cfVersion: ContentVersion = {
      id: "5800999",
      project_id: "999001",
      name: "1.0.0",
      version_number: null,
      version_type: "release",
      game_versions: ["1.21.4"],
      loaders: ["fabric"],
      date_published: "2024-12-12T00:00:00Z",
      file_name: "cf-restricted-1.0.0.jar",
      file_size: 4096,
      manual_download: true,
      download_page_url: "https://www.curseforge.com/minecraft/mc-mods/restricted-mod/download/5800999",
    };

    const fetchMock = mockFetch((url, init) => {
      if (url === "/api/v1/servers/A/content" && (!init?.method || init.method === "GET")) {
        return Response.json({ kind: "mods", files: [], pending: [] });
      }
      if (url === "/api/v1/settings/curseforge") return Response.json({ available: true, source: "user" });
      if (url.startsWith("/api/v1/content/search")) {
        const source = new URL(url, "http://localhost").searchParams.get("source");
        return Response.json({ hits: source === "curseforge" ? [cfHit] : [lithiumHit], page: 1, page_size: 20, total: 1 });
      }
      if (url === "/api/v1/content/curseforge/projects/999001/versions?server_id=A") return Response.json([cfVersion]);
      if (url === "/api/v1/servers/A/content/plan" && init?.method === "POST") {
        return Response.json({
          kind: "mods",
          items: [
            {
              project_id: "999001",
              project_name: "Restricted Mod",
              version_id: "5800999",
              version_number: "1.0.0",
              file_name: "cf-restricted-1.0.0.jar",
              file_size: 4096,
              client_only: false,
              replaces_existing: false,
              is_dependency: false,
              manual_download: true,
              download_page_url: "https://www.curseforge.com/minecraft/mc-mods/restricted-mod/download/5800999",
            },
          ],
        } satisfies ContentPlan);
      }
      if (url === "/api/v1/servers/A/content" && init?.method === "POST") return Response.json({ job_id: 12 }, { status: 202 });
      if (url === "/api/v1/jobs/12") return Response.json(runningJob);
      return undefined;
    });

    const user = userEvent.setup();
    renderPage(stoppedFabric, "/servers/A/content?add=1");
    const dialog = await screen.findByRole("dialog");

    const cfRadio = within(dialog).getByRole("radio", { name: "CurseForge" });
    await waitFor(() => expect(cfRadio).toHaveProperty("disabled", false));
    await user.click(cfRadio);

    expect(within(dialog).getByRole("searchbox", { name: "Tìm mod trên CurseForge…" })).toBeTruthy();
    await user.click(await within(dialog).findByRole("button", { name: /Restricted Mod/ }));

    expect(await within(dialog).findByText("Cần tải tay")).toBeTruthy();
    expect(within(dialog).getByText(/chặn tải tự động qua API/)).toBeTruthy();
    const link = within(dialog).getByRole("link", { name: "Mở trang tải" });
    expect(link.getAttribute("href")).toBe("https://www.curseforge.com/minecraft/mc-mods/restricted-mod/download/5800999");

    await user.click(within(dialog).getByRole("button", { name: "Cài" }));
    await waitFor(() => {
      const post = fetchMock.mock.calls.find(([u, i]) => u === "/api/v1/servers/A/content" && i?.method === "POST");
      expect(JSON.parse(String(post?.[1]?.body))).toEqual({
        source: "curseforge",
        project_id: "999001",
        version_id: "5800999",
      });
    });
  });
});
